#!/usr/bin/env python3
"""Build a Horizon Android APK with native mods (.hnmod) compiled in.

A .hnmod is Rust source (horizon-mixins bundles), never a plugin: it plays only in a
game compiled with it, and a phone has no compiler. This tool does what the desktop
launcher's native build does, for the Android target:

  check  MOD...            read each .hnmod as the registry does and say what a build would do
  stage  MOD... --out DIR  apply the mods with horizon-mixins into DIR, a modded copy of this tree
                           (--in-place: then copy the changes into this checkout, for CI)
  build  MOD... [-- ARGS]  stage, then run tools/android_build.py in the copy (ARGS go to it)

Making your own mod:

  new ID [--dir DIR]       start a mod from a template that uses only the native hook points
  pack FOLDER [--out FILE] package your mod folder as a .hnmod (import it in the app, share it)

A mod folder (a horizon-mixin.toml and its .rs files) also works as MOD for check, stage
and build, so you can build an APK from your work in progress.

MOD is a .hnmod file, an unpacked .hnmod folder, your mod's folder, or an https:// link.
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
    # A mod folder in the making: no manifest yet, built on this source.
    local: bool = False

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


def folder_files(path: Path) -> dict[str, bytes]:
    """A folder's files, without build output (target/) or hidden files and folders."""
    files = {}
    for item in sorted(path.rglob('*')):
        rel = item.relative_to(path)
        if rel.parts[0] == 'target' or any(part.startswith('.') and part != '.gitignore' for part in rel.parts):
            continue
        if item.is_symlink():
            raise ModError(f'{path}: links are refused: {item}')
        if item.is_file():
            files[rel.as_posix()] = item.read_bytes()
    return files


def read_mixin(path: Path) -> dict:
    try:
        toml = tomllib.loads((path / 'horizon-mixin.toml').read_text(encoding='utf-8'))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        raise ModError(f'{path}: horizon-mixin.toml: {e}') from None
    mod_id = toml.get('mod', {}).get('id', '')
    if not isinstance(mod_id, str) or not re.fullmatch(r'[a-z0-9_]{1,64}', mod_id) \
            or not toml.get('mod', {}).get('version'):
        raise ModError(f'{path}: horizon-mixin.toml needs [mod] id (a-z, 0-9, _) and version')
    return toml


def read_local(path: Path) -> Mod:
    """Your mod folder (horizon-mixin.toml at its root): built on this source, as it is."""
    toml = read_mixin(path)
    mod_id = toml['mod']['id']
    files = {f'mixins/{mod_id}/{name}': content for name, content in folder_files(path).items()}
    horizon = str(toml.get('compatibility', {}).get('horizon_revision', ''))
    return Mod(mod_id, str(toml['mod']['version']), '', horizon, '', None, files, str(path), toml, local=True)


def read_folder(path: Path) -> Mod:
    if not (path / MANIFEST).is_file() and (path / 'horizon-mixin.toml').is_file():
        return read_local(path)
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
        if mod.game != tree.game and not mod.local:
            raise ModError(f'{mod.label} is for Minecraft {mod.game}; this source is {tree.game}')
        if why := android_blocker(mod):
            raise ModError(f'{mod.label} cannot be built for Android: {why}')
        if mod.hooks is not None and mod.hooks > tree.hooks:
            raise ModError(f'{mod.label} needs native hook points version {mod.hooks}; '
                           f'this source has {tree.hooks}: merge a newer upstream first')
        if mod.hooks is None and not (mod.local and not mod.horizon):
            pinned.append(mod)
        build.append(mod)
    return Plan(build, skipped, pinned)


def describe(result: Plan, tree: Tree) -> None:
    print(f'Source: Minecraft {tree.game}, native hook points {tree.hooks}')
    for mod in result.build:
        how = ('your folder, built on this source' if mod.local and not mod.horizon
               else 'hook points only' if mod.hooks is not None
               else f'pinned to Horizon {mod.horizon[:12]}')
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
    rev = newest(cache, {tree.mixins_rev, *(m.mixins for m in result.build if m.mixins)})
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


TEMPLATE = {
    'horizon-mixin.toml': """[mod]
id = "{id}"
version = "0.1.0"
priority = 100

# It hooks only Horizon's native hook points (crates/horizon-client/src/game/native_hooks.rs),
# so it builds on any Horizon that has them. `android_native_mods.py pack` adds the
# [compatibility] commit and the .hnmod manifest. docs/native-mods.md lists every hook.

# Your own code: the game gets it as the module crate::game::mod_{id}.
[[module]]
id = "runtime"
file = "crates/horizon-client/src/game.rs"
name = "mod_{id}"
source = "runtime.rs"

# Its row in Options > Mods, with its On/Off switch.
[[injection]]
id = "list"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_mods"
helper = "list.rs"
bindings = [
  {{ name = "mods", ty = "&mut Vec<crate::game::native_hooks::NativeMod>", value = "&mut *mods" }},
]

# A box on the HUD.
[[injection]]
id = "hud"
kind = "HEAD"
target = "horizon_client::game::native_hooks::Game<'_>::native_hud"
helper = "hud.rs"
bindings = [
  {{ name = "game", ty = "&mut crate::game::Game<'_>", value = "&mut *self" }},
  {{ name = "batch", ty = "&mut horizon_client::hud::HudBatch", value = "&mut *batch" }},
  {{ name = "corners", ty = "&mut horizon_client::extras_hud::Corners", value = "&mut *corners" }},
]
""",
    'runtime.rs': """//! {title}: the mod's own code, which the game gets as `crate::game::mod_{id}`.

/// The mod's id (its settings folder), name and version.
pub const ID: &str = "{id}";
pub const NAME: &str = "{title}";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "A native mod made from the Horizon Android template.";
/// The box: translucent black (ARGB) and white text (RGB).
pub const BACKGROUND: u32 = 0x7800_0000;
pub const INK: u32 = 0xFF_FFFF;

/// What the box says, from the frames drawn in the last second.
pub fn text(fps: u32) -> String {{
    format!("{{NAME}}: {{fps}} FPS")
}}
""",
    'list.rs': """// The mod's row in Options > Mods.
fn mod_{id}_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {{
    use crate::game::mod_{id} as m;
    mods.push(crate::game::native_hooks::NativeMod {{
        id: m::ID.into(),
        title: format!("{{}} {{}}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: Vec::new(),
    }});
}}
""",
    'hud.rs': """// A box in the top right corner while the mod is on.
fn mod_{id}_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {{
    use crate::game::mod_{id} as m;
    use horizon_client::extras::Corner;
    // Turned off in Options > Mods, or the corner is taken (the debug screen).
    if !game.native_on(m::ID) || !game.native_corner_shown(Corner::TopRight) {{
        return;
    }}
    let text = m::text(game.native_frame_rate().0);
    let w = game.native_text_width(&text) + 4.0;
    let (x, y) = corners.place(Corner::TopRight, w, 12.0);
    game.native_fill(batch, [x, y, w, 12.0], m::BACKGROUND);
    game.native_text(batch, &text, [x + 2.0, y + 2.0], m::INK, true);
}}
""",
}


def new_mod(mod_id: str, folder: Path) -> Path:
    """A mod folder from the template."""
    if not re.fullmatch(r'[a-z][a-z0-9_]{0,63}', mod_id):
        raise ModError('a mod id is lowercase letters, digits and _, starting with a letter')
    if folder.exists() and any(folder.iterdir()):
        raise ModError(f'{folder} is not empty')
    title = mod_id.replace('_', ' ').title()
    folder.mkdir(parents=True, exist_ok=True)
    for name, text in TEMPLATE.items():
        (folder / name).write_text(text.format(id=mod_id, title=title), encoding='utf-8')
    return folder


def only_hook_points(toml: dict) -> bool:
    """Whether every injection is on a native hook point (a hook name or a native_hooks function)."""
    injections = toml.get('injection', [])
    return bool(injections) and all(
        'hook' in i or '::native_hooks::' in str(i.get('target', '')) for i in injections)


def head_commit(root: Path) -> str:
    try:
        rev = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        raise ModError('pack needs this source to be a git checkout (its commit is the one tested)') from None
    if not REVISION.fullmatch(rev):
        raise ModError(f'unexpected commit {rev!r}')
    return rev


def pack(folder: Path, out: Path | None, tree: Tree, hooks: int | None) -> Path:
    """Your mod folder as a .hnmod: the manifest, and the commit it was tested on in its mixin."""
    if (folder / MANIFEST).is_file():
        raise ModError(f'{folder} is an unpacked .hnmod already: zip it as it is')
    toml = read_mixin(folder)
    mod_id, version = toml['mod']['id'], str(toml['mod']['version'])
    text = (folder / 'horizon-mixin.toml').read_text(encoding='utf-8')
    horizon = str(toml.get('compatibility', {}).get('horizon_revision', ''))
    if not horizon:
        horizon = head_commit(ROOT)
        text = text.rstrip('\n') + f'\n\n[compatibility]\nhorizon_revision = "{horizon}"\n'
    elif not REVISION.fullmatch(horizon):
        raise ModError(f'{folder}: horizon_revision must be a full 40-character commit')
    if hooks is None and only_hook_points(toml):
        hooks = tree.hooks
    manifest = {'game': tree.game, 'horizon': horizon, 'id': mod_id, 'mixins': tree.mixins_rev,
                'schema': 1, 'version': version}
    if hooks is not None:
        manifest['hooks'] = hooks
    files = {MANIFEST: (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode()}
    for name, content in folder_files(folder).items():
        files[f'mixins/{mod_id}/{name}'] = text.encode() if name == 'horizon-mixin.toml' else content
    read_bundle(files, str(folder))  # the checks the app and the registry make
    out = out or folder.parent / f'{mod_id}-{version}.hnmod'
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in sorted(files):
            archive.writestr(zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)), files[name],
                             compress_type=zipfile.ZIP_DEFLATED)
    out.write_bytes(buffer.getvalue())
    read_zip(out.read_bytes(), str(out))
    return out


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    passthrough: list[str] = []
    if '--' in argv:
        index = argv.index('--')
        argv, passthrough = argv[:index], argv[index + 1:]
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['check', 'stage', 'build', 'new', 'pack'])
    parser.add_argument('mods', nargs='+', metavar='MOD',
                        help='.hnmod file, unpacked .hnmod, mod folder or https:// link (new: the mod id; pack: the folder)')
    parser.add_argument('--dir', type=Path, help='new: the folder to create (default: ./ID)')
    parser.add_argument('--hooks', type=int,
                        help='pack: the native hook points version it needs (default: this source\'s, '
                             'when it hooks nothing else)')
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
        if args.command == 'new':
            if len(args.mods) != 1:
                raise ModError('new takes one mod id')
            folder = new_mod(args.mods[0], (args.dir or Path(args.mods[0])).resolve())
            print(f'Created {folder}. Edit runtime.rs and hud.rs, then:')
            print(f'  python3 tools/android_native_mods.py pack {folder}')
            print(f'  python3 tools/android_native_mods.py build {folder} --trust-native-code -- --abi arm64-v8a')
            return 0
        if args.command == 'pack':
            if len(args.mods) != 1:
                raise ModError('pack takes one mod folder')
            if args.hooks is not None and not 1 <= args.hooks <= tree.hooks:
                raise ModError(f'--hooks must be 1..{tree.hooks}')
            out = args.out if args.out != parser.get_default('out') else None
            written = pack(Path(args.mods[0]).resolve(), out, tree, args.hooks)
            print(f'Wrote {written}: import it in the app (Mods > Import file) or share it')
            return 0
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
