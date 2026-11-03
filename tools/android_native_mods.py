#!/usr/bin/env python3
"""Build a Horizon Android APK with native mods (.hnmod) compiled in.

A .hnmod is Rust source (horizon-mixins bundles), never a plugin: it plays only in a
game compiled with it, and a phone has no compiler. This tool does what the desktop
launcher's native build does, for the Android target:

  check  MOD...            read each .hnmod as the registry does and say what a build would do
  stage  MOD... --out DIR  apply the mods with horizon-mixins into DIR, a modded copy of this tree
                           (--in-place: then copy the changes into this checkout, for CI)
  build  MOD... [-- ARGS]  stage, then run tools/android_build.py in the copy (ARGS go to it)

MOD is a .hnmod file, an unpacked .hnmod folder, or an https:// link to a .hnmod.
Mods whose id this APK already bundles (Zoom, Minimap, Recipes, SmartF3) are
skipped: the fixed Android copy plays. A mod that hooks only Horizon's native hook
points (`hooks` in its manifest) builds on any source with that hook version; any
other mod was made for one exact Horizon commit and needs --allow-version-mismatch,
as the launcher's "Build anyway". Native mods are native code with your rights:
build only mods you trust (--trust-native-code).

The built APK records the mods (HORIZON_ANDROID_NATIVE_MODS), so the launcher's
Installed > Native Mods says "Built in" for them. See docs/android-native-mods.md.
"""
from __future__ import annotations

import argparse
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
import urllib.request
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'horizon-native.json'
MIXINS_GIT = 'https://github.com/horizon-rust/horizon-mixins'
# horizon-mixins' own file in a stage, and this tool's record of what a stage holds.
STAGE_MARKER = '.horizon-mixins-stage.json'
RECORD = '.horizon-native-mods.json'
MAX_BUNDLE = 64 << 20
MAX_ENTRIES = 512
REVISION = re.compile(r'[0-9a-f]{40}')
TEXT_PREFIXES = ('README', 'LICENSE', 'LICENCE', 'COPYING', 'NOTICE')


class ModError(Exception):
    pass


@dataclass
class Mod:
    id: str
    version: str
    game: str
    horizon: str
    mixins: str
    hooks: int | None
    files: dict[str, bytes] = field(repr=False)
    source: str = ''
    mixin: dict = field(default_factory=dict, repr=False)

    @property
    def label(self) -> str:
        return f'{self.id} {self.version}'


def allowed(rest: str) -> bool:
    """Whether a bundle may carry `rest` (its path under mixins/<id>/), as horizon_mods' native index."""
    parts = rest.split('/')
    data = len(parts) >= 2 and (parts[0] == 'data' or (len(parts) >= 3 and parts[1] == 'data'))
    name = parts[-1]
    text = len(parts) <= 2 and (name.startswith(TEXT_PREFIXES) or name == '.gitignore')
    return rest.endswith('.rs') or rest.endswith('.toml') or data or text


def read_bundle(files: dict[str, bytes], source: str) -> Mod:
    """Checks an unpacked .hnmod: only sources for its own id, its manifest and mixin identity."""
    if MANIFEST not in files:
        raise ModError(f'{source}: missing {MANIFEST}')
    try:
        manifest = json.loads(files[MANIFEST])
    except ValueError as e:
        raise ModError(f'{source}: {MANIFEST}: {e}') from None
    keys = {'schema', 'id', 'version', 'game', 'horizon', 'mixins', 'hooks'}
    if not isinstance(manifest, dict) or set(manifest) - keys:
        raise ModError(f'{source}: unknown fields in {MANIFEST}')
    mod_id = manifest.get('id', '')
    hooks = manifest.get('hooks')
    if (manifest.get('schema') != 1
            or not isinstance(mod_id, str) or not re.fullmatch(r'[a-z0-9_]{1,64}', mod_id)
            or not manifest.get('version') or not manifest.get('game')
            or not REVISION.fullmatch(str(manifest.get('horizon', '')))
            or not REVISION.fullmatch(str(manifest.get('mixins', '')))
            or (hooks is not None and (not isinstance(hooks, int) or hooks < 1))):
        raise ModError(f'{source}: invalid native manifest: schema 1, id, version, game and exact revisions required')
    prefix = f'mixins/{mod_id}/'
    for name in files:
        if name == MANIFEST:
            continue
        if not name.startswith(prefix) or not allowed(name[len(prefix):]):
            raise ModError(f'{source}: unexpected native source: {name}')
    mixin = files.get(prefix + 'horizon-mixin.toml')
    if mixin is None:
        raise ModError(f'{source}: missing {prefix}horizon-mixin.toml')
    try:
        toml = tomllib.loads(mixin.decode())
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        raise ModError(f'{source}: horizon-mixin.toml: {e}') from None
    if (toml.get('mod', {}).get('id') != mod_id
            or toml.get('mod', {}).get('version') != manifest['version']
            or toml.get('compatibility', {}).get('horizon_revision') != manifest['horizon']):
        raise ModError(f'{source}: native manifest and mixin identity/revision differ')
    return Mod(mod_id, manifest['version'], manifest['game'], manifest['horizon'],
               manifest['mixins'], hooks, files, source, toml)


def safe_name(name: str) -> bool:
    parts = name.split('/')
    return (bool(name) and not name.startswith('/') and '\\' not in name and ':' not in name
            and all(p not in ('', '.', '..') for p in parts))


def read_zip(data: bytes, source: str) -> Mod:
    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile:
        raise ModError(f'{source}: not a .hnmod archive') from None
    entries = archive.infolist()
    if len(entries) > MAX_ENTRIES:
        raise ModError(f'{source}: native bundle has too many entries')
    files: dict[str, bytes] = {}
    total = 0
    for entry in entries:
        if entry.is_dir():
            continue
        mode = entry.external_attr >> 16
        if (mode & 0o170000) not in (0, 0o100000) or not safe_name(entry.filename):
            raise ModError(f'{source}: invalid native bundle path: {entry.filename}')
        with archive.open(entry) as handle:
            content = handle.read(MAX_BUNDLE + 1)
        total += len(content)
        if total > MAX_BUNDLE:
            raise ModError(f'{source}: native bundle expands beyond 64 MiB')
        if entry.filename in files:
            raise ModError(f'{source}: duplicate native bundle path')
        files[entry.filename] = content
    return read_bundle(files, source)


def read_folder(path: Path) -> Mod:
    files = {}
    for item in sorted(path.rglob('*')):
        if item.is_symlink():
            raise ModError(f'{path}: links are refused: {item}')
        if item.is_file():
            files[item.relative_to(path).as_posix()] = item.read_bytes()
    return read_bundle(files, str(path))


def load(spec: str) -> Mod:
    if spec.startswith('https://'):
        request = urllib.request.Request(spec, headers={'User-Agent': 'horizon-android-native-mods'})
        with urllib.request.urlopen(request, timeout=90) as response:
            data = response.read(MAX_BUNDLE + 1)
        if len(data) > MAX_BUNDLE:
            raise ModError(f'{spec}: larger than 64 MiB')
        return read_zip(data, spec)
    if spec.startswith('http://'):
        raise ModError(f'{spec}: only https:// links are downloaded')
    path = Path(spec)
    if path.is_dir():
        return read_folder(path)
    if path.is_file():
        return read_zip(path.read_bytes(), spec)
    raise ModError(f'{spec}: no such file')


@dataclass
class Tree:
    """What a source tree offers native mods."""
    game: str
    hooks: int
    bundled: list[str]
    mixins_rev: str

    @staticmethod
    def read(root: Path) -> 'Tree':
        def find(path: str, pattern: str) -> str:
            match = re.search(pattern, (root / path).read_text(encoding='utf-8'), re.S)
            if not match:
                raise ModError(f'{path}: cannot find {pattern!r}')
            return match.group(1)
        game = find('crates/horizon-core/src/version.rs', r'pub const VERSION_ID: &str = "([^"]+)"')
        hooks = int(find('crates/horizon-client/src/game/native_hooks.rs',
                         r'pub\(crate\) const NATIVE_HOOKS_API: u32 = (\d+);'))
        listing = find('crates/horizon-client-render/src/mod_list.rs',
                       r'pub const ANDROID_BUNDLED: \[\(&str, &str, &str\); \d+\] = \[(.*?)\n\];')
        bundled = re.findall(r'\(\s*"([a-z0-9_]+)"', listing)
        mixins_rev = find('launcher/src/native_mods.rs', r'pub const MIXINS_REV: &str = "([0-9a-f]{40})"')
        return Tree(game, hooks, bundled, mixins_rev)


@dataclass
class Plan:
    build: list[Mod]
    skipped: list[tuple[Mod, str]]
    pinned: list[Mod]

    @property
    def env(self) -> str:
        return ','.join(f'{m.id}@{m.version}' for m in self.build)


# Game files the Android library does not compile: its root is android_main.rs, not the
# desktop binary's main.rs, and horizon-ctl is a desktop tool.
DESKTOP_ONLY = ('crates/horizon-client/src/main.rs', 'crates/horizon-client/src/bin/')


def android_blocker(mod: Mod) -> str | None:
    """Why `mod` cannot play in an APK, whatever source it is applied to."""
    if mod.mixin.get('post_build'):
        return ('it has post-build steps, which run in a desktop version folder; '
                'an APK has none')
    for module in mod.mixin.get('module', []):
        file = str(module.get('file', ''))
        if file.startswith(DESKTOP_ONLY):
            return f'it adds a module to {file}, which the Android library does not compile'
    return None


def plan(mods: list[Mod], tree: Tree) -> Plan:
    """Which mods a build compiles, which it skips and why, and which pin another Horizon commit."""
    seen: set[str] = set()
    build, skipped, pinned = [], [], []
    for mod in mods:
        if mod.id in seen:
            raise ModError(f'{mod.id} is given twice: keep one version')
        seen.add(mod.id)
        if mod.id in tree.bundled:
            skipped.append((mod, 'this APK bundles its fixed Android copy, which plays instead'))
            continue
        if mod.game != tree.game:
            raise ModError(f'{mod.label} is for Minecraft {mod.game}; this source is {tree.game}')
        if why := android_blocker(mod):
            raise ModError(f'{mod.label} cannot be built for Android: {why}')
        if mod.hooks is not None and mod.hooks > tree.hooks:
            raise ModError(f'{mod.label} needs native hook points version {mod.hooks}; '
                           f'this source has {tree.hooks}: merge a newer upstream first')
        if mod.hooks is None:
            pinned.append(mod)
        build.append(mod)
    return Plan(build, skipped, pinned)


def describe(result: Plan, tree: Tree) -> None:
    print(f'Source: Minecraft {tree.game}, native hook points {tree.hooks}')
    for mod in result.build:
        how = 'hook points only' if mod.hooks is not None else f'pinned to Horizon {mod.horizon[:12]}'
        print(f'  build  {mod.label}  ({how})')
    for mod, why in result.skipped:
        print(f'  skip   {mod.label}: {why}')


def run(command: list, **kwargs) -> None:
    print('+', ' '.join(map(str, command)), flush=True)
    subprocess.run(list(map(str, command)), check=True, **kwargs)


def source_files(root: Path) -> list[str]:
    """This checkout's files (tracked and new, not ignored): never .local, target or .git."""
    out = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'],
                                  cwd=root)
    return sorted({name for name in out.decode().split('\0') if name and (root / name).is_file()})


def copy_tree(root: Path, files: list[str], dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    for name in files:
        target = dest / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / name, target)


def newest(cache: Path, revisions: set[str]) -> str:
    """The newest of the horizon-mixins commits: never one older than a mod was made with."""
    repo = cache / 'horizon-mixins.git'
    if not repo.is_dir():
        run(['git', 'clone', '--bare', '--filter=blob:none', MIXINS_GIT, repo])
    else:
        run(['git', '--git-dir', repo, 'fetch', '--quiet', 'origin', '+refs/heads/*:refs/heads/*'])
    best = None
    for rev in sorted(revisions):
        if subprocess.run(['git', '--git-dir', str(repo), 'cat-file', '-e', rev + '^{commit}']).returncode:
            raise ModError(f'horizon-mixins has no commit {rev}')
        if best is None or subprocess.run(['git', '--git-dir', str(repo), 'merge-base', '--is-ancestor',
                                           best, rev]).returncode == 0:
            best = rev
        elif subprocess.run(['git', '--git-dir', str(repo), 'merge-base', '--is-ancestor',
                             rev, best]).returncode != 0:
            raise ModError(f'horizon-mixins commits {best[:12]} and {rev[:12]} diverge')
    return best


def mixins_tool(cache: Path, rev: str) -> Path:
    root = cache / 'tools' / rev[:12]
    binary = root / 'bin' / ('horizon-mixins' + ('.exe' if os.name == 'nt' else ''))
    if not binary.is_file():
        run(['cargo', 'install', '--locked', '--git', MIXINS_GIT, '--rev', rev, '--root', root,
             'horizon-mixins-cli'])
    return binary


def stage(result: Plan, tree: Tree, out: Path, cache: Path) -> None:
    """The mods applied to a copy of this checkout, into `out`."""
    work = cache / 'work'
    mods = work / 'mods'
    if mods.exists():
        shutil.rmtree(mods)
    for mod in result.build:
        for name, content in mod.files.items():
            if name == MANIFEST:
                continue
            target = mods / name[len('mixins/'):]
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
    source = work / 'source'
    copy_tree(ROOT, source_files(ROOT), source)
    rev = newest(cache, {tree.mixins_rev, *(m.mixins for m in result.build)})
    tool = mixins_tool(cache, rev)
    # horizon-mixins reads the workspace with offline, locked cargo metadata.
    run(['cargo', 'fetch', '--locked', '--manifest-path', source / 'Cargo.toml'])
    out.parent.mkdir(parents=True, exist_ok=True)
    if out.exists():
        # horizon-mixins refuses links in a stage and replaces a stage whole (Linux only).
        if (out / '.local').is_symlink():
            (out / '.local').unlink()
        (out / RECORD).unlink(missing_ok=True)
        if not sys.platform.startswith('linux'):
            shutil.rmtree(out)
    run([tool, 'apply', '--workspace', source, '--mods', mods, '--out', out])
    # The build reads the prepared resources and the signing key from this checkout's .local.
    local = out / '.local'
    if not local.exists():
        (ROOT / '.local').mkdir(exist_ok=True)
        try:
            local.symlink_to(ROOT / '.local', target_is_directory=True)
        except OSError as e:
            raise ModError(f'cannot link .local into the stage ({e}); on Windows enable '
                           'Developer Mode or build from WSL') from None
    (out / RECORD).write_text(json.dumps({
        'mods': [{'id': m.id, 'version': m.version, 'horizon': m.horizon, 'mixins': m.mixins,
                  'hooks': m.hooks} for m in result.build],
        'horizon_mixins': rev,
    }, indent=2) + '\n')


def copy_back(out: Path, root: Path) -> list[str]:
    """The stage's changed and new files copied over this checkout (a CI checkout is disposable)."""
    changed = []
    for path in sorted(out.rglob('*')):
        name = path.relative_to(out).as_posix()
        if name.split('/')[0] in ('.local', STAGE_MARKER, RECORD) or path.is_symlink() or not path.is_file():
            continue
        target = root / name
        if not target.is_file() or target.read_bytes() != path.read_bytes():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            changed.append(name)
    return changed


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    passthrough: list[str] = []
    if '--' in argv:
        index = argv.index('--')
        argv, passthrough = argv[:index], argv[index + 1:]
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['check', 'stage', 'build'])
    parser.add_argument('mods', nargs='+', metavar='MOD', help='.hnmod file, unpacked folder or https:// link')
    parser.add_argument('--out', type=Path, default=ROOT / '.local/android-native/stage',
                        help='the modded copy of this tree (stage, build)')
    parser.add_argument('--cache', type=Path, default=ROOT / '.local/android-native',
                        help='horizon-mixins and the work folders')
    parser.add_argument('--allow-version-mismatch', action='store_true',
                        help='also build mods pinned to another Horizon commit (they often still apply)')
    parser.add_argument('--trust-native-code', action='store_true',
                        help='required for stage and build: native mods run with your rights')
    parser.add_argument('--in-place', action='store_true',
                        help='stage only: copy the modded files into this checkout (CI)')
    parser.add_argument('--github-env', type=Path,
                        help='append HORIZON_ANDROID_NATIVE_MODS to this file (GitHub Actions $GITHUB_ENV)')
    args = parser.parse_args(argv)
    try:
        tree = Tree.read(ROOT)
        result = plan([load(spec) for spec in args.mods], tree)
        describe(result, tree)
        if args.command == 'check':
            return 0
        if not result.build:
            raise ModError('nothing to build: every mod given is already in this APK')
        if not args.trust_native_code:
            raise ModError('native mods are native code with your rights: pass --trust-native-code '
                           'to build mods you trust')
        if result.pinned and not args.allow_version_mismatch:
            names = ', '.join(f'{m.label} (Horizon {m.horizon[:12]})' for m in result.pinned)
            raise ModError(f'{names}: made for one Horizon commit, not this Android source. They often '
                           'still apply: pass --allow-version-mismatch to build anyway')
        out = args.out.resolve()
        if out == ROOT or (ROOT in out.parents and ROOT / '.local' not in out.parents):
            raise ModError('--out must be outside this checkout or under its .local/')
        if args.in_place and args.command != 'stage':
            raise ModError('--in-place is for stage')
        cache = args.cache.resolve()
        stage(result, tree, out, cache)
        if args.in_place:
            changed = copy_back(out, ROOT)
            print(f'Copied {len(changed)} modded file(s) into this checkout')
        if args.github_env:
            with args.github_env.open('a') as env:
                env.write(f'HORIZON_ANDROID_NATIVE_MODS={result.env}\n')
        print(f'Staged {len(result.build)} native mod(s) in {out}')
        if args.command == 'build':
            environment = dict(os.environ, HORIZON_ANDROID_NATIVE_MODS=result.env)
            # Outside the stage: horizon-mixins replaces the stage whole on the next run.
            environment.setdefault('CARGO_TARGET_DIR', str(cache / 'target'))
            run([sys.executable, out / 'tools/android_build.py', *passthrough], cwd=out, env=environment)
            print('APK: .local/android/apk/ (this build has the native mods compiled in)')
    except ModError as e:
        print(f'error: {e}', file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as e:
        print(f'error: {e}', file=sys.stderr)
        return e.returncode or 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
