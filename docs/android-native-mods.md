# Native mods (.hnmod) on Android

A `.hnmod` is a [native mod](native-mods.md): Rust source (a horizon-mixins bundle) that
changes the game before it is compiled. It is not a plugin, and a phone has no compiler,
so a native mod plays only in an APK built with it. Sandboxed `.hmod` mods still install
and run without a rebuild.

## In the launcher

- **Get one.** The Mods tab lists the registry's native mods with an `.hnmod` badge and a
  **Download** button. You can also paste an HTTPS link to a `.hnmod` in the search
  field and press **Import mod**, or use **Import file** with a `.hnmod` from your phone.
  An imported `.hnmod` goes to Installed > Native Mods, never into the sandboxed mods.
- **Checks.** Every `.hnmod` is checked the way the registry checks it: only Rust, TOML,
  data and licence files for its own mod id, an exact Horizon and horizon-mixins commit,
  and a mixin manifest that matches it. Nothing in it runs on the phone. A file with the
  same mod id replaces the older one: **Keep both** is not offered.
- **Status.** Each row of Installed > Native Mods says what this APK does with the mod:

  | Badge | Meaning |
  |---|---|
  | **Built in** | This APK was compiled with this version: it plays, always on (native mods have no switch). |
  | **Build has X** | This APK was compiled with version X of this mod. |
  | **Needs build** | Build an APK with it (below). |
  | **Bundled copy** | The APK's fixed Android copy (Zoom, Minimap, Recipes, SmartF3) plays instead; enable it in the Mods tab. |
  | **For Minecraft X**, **Needs newer Horizon**, **Desktop only** | Made for another game version, for native hook points this source does not have yet, or with desktop-only parts ([below](#which-mods-work)). |

- **Files.** Android Files shows the profile's **Native Mods** folder
  (`horizon-native-mods/`) next to Mods, Resource Packs and Shader Packs.

## Making your own mod

```sh
python3 tools/android_native_mods.py new my_mod            # a mod folder from the template
python3 tools/android_native_mods.py build my_mod --trust-native-code -- --abi arm64-v8a
python3 tools/android_native_mods.py pack my_mod           # my_mod-0.1.0.hnmod, to import or share
```

`new` writes a mod that hooks only Horizon's native hook points, so it builds on any
Horizon with them:

| File | What it is |
|---|---|
| `horizon-mixin.toml` | The mod: its id and version, its module and the hooks it uses |
| `runtime.rs` | Your code; the game gets it as the module `crate::game::mod_<id>` |
| `list.rs` | Its row in Options > Mods (on the `native_mods` hook), with its On/Off switch |
| `hud.rs` | A box in the HUD's top right corner (on the `native_hud` hook) showing the FPS |

A helper is compiled as a local function at the start (`HEAD`) of the hook it names and
gets the `bindings` listed for it. The hook points and their helpers (`native_on`,
`native_text`, `native_setting`, blocks, items, commands, keys...) are documented in
`crates/horizon-client/src/game/native_hooks.rs` and [native-mods.md](native-mods.md);
[horizon-mixins](https://github.com/horizon-rust/horizon-mixins) lists the other injection
kinds. Installed mods such as fps_overlay or fullbright are good examples.

`build` takes your folder as it is, built on this source. `pack` writes the `.hnmod`: the
manifest, with this checkout's commit as the one it was tested on (added to the mixin's
`[compatibility]`) and `hooks` set to this source's hook version when it hooks only hook
points (`--hooks N` to name a lower one). It runs the app's checks on the result, so a
packed mod imports. Build output (`target/`) and hidden files are left out.

## Building an APK with native mods

Native mods are native code with your rights: build only mods you trust. On a computer set
up for the [Android build](android.md#build-developers-only):

```sh
python3 tools/android_native_mods.py check shulker_peek-0.1.0.hnmod zoom-2.0.0.hnmod
python3 tools/android_native_mods.py build shulker_peek-0.1.0.hnmod --trust-native-code \
    --allow-version-mismatch -- --abi arm64-v8a
```

`check` reads the mods and prints what a build would do, with no trust needed. `build`:

1. reads each `.hnmod` (a file, an unpacked folder, your mod folder or an `https://` link)
   as above;
2. skips a mod whose id this APK bundles: its fixed Android copy plays instead;
3. stops when a mod is for another Minecraft version or needs newer native hook points
   (`hooks` in its manifest) than this source has (`NATIVE_HOOKS_API`);
4. installs horizon-mixins at the newest of the commits the mods and the launcher name,
   with `cargo install --locked`, into `.local/android-native/tools/`;
5. copies this checkout (tracked and new files, never `.local/`, `target/` or `.git/`) and
   applies the mods with `horizon-mixins apply` into `.local/android-native/stage/`;
6. runs that copy's `tools/android_build.py` with the arguments after `--`, and
   `HORIZON_ANDROID_NATIVE_MODS=id@version,...` so the launcher knows what it holds.
   The build reads `.local/` (prepared resources, signing key) from this checkout, and
   compiles into `.local/android-native/target/` unless `CARGO_TARGET_DIR` is set.

The APK is written to `.local/android/apk/<abi>/`, as a normal build. Signed with the same
key, it updates the installed app and keeps its worlds.

A mod that hooks only Horizon's native hook points (its manifest has `hooks`) builds on
any source with that hook version. Any other mod was made for one exact Horizon commit,
and this Android fork is never that commit: `--allow-version-mismatch` builds it anyway,
like the desktop launcher's **Build anyway**. Such mods often still apply; when one does
not, horizon-mixins names the hook that no longer matches.

`stage` applies the mods without building. With `--in-place` it then copies the modded
files over this checkout, for a disposable CI checkout.

### On GitHub Actions

The **Android dev APKs** workflow has a **native_mods** input: space-separated HTTPS
links to `.hnmod` files. A run with it compiles those mods into the dev APK
(`stage --in-place --trust-native-code --allow-version-mismatch`). Only people who can
run the workflow can choose mods, and the APK is signed with the dev key: give it only
mods you trust.

## Which mods work

Any valid `.hnmod` imports and shows its status. Building one into an APK works when:

- it is for this game version (Minecraft 26.3);
- it hooks only the native hook points (`hooks` in its manifest) at this source's version or
  lower: the most portable kind, independent of the commit it was made on;
- or, pinned to one Horizon commit, horizon-mixins still finds every function it hooks in
  this fork (`--allow-version-mismatch`).

These cannot be built for Android, whatever the source: mods with `[[post_build]]` steps
(they run in a desktop version folder; an APK has none) and mods that add a module to the
desktop binary's own files (`crates/horizon-client/src/main.rs`, `src/bin/`), which the
Android library does not compile. Installed says **Desktop only** for them, and the build
tool refuses them by name. Mods with their own crates and data files are staged as on
desktop; their crates must compile for Android.

## Limits

- No on-phone compiler and no dynamic loading: changing the mods means a new APK.
- A mod pinned to another Horizon commit may fail to apply or to compile on this fork.
- The fixed Android bundle wins over a `.hnmod` of the same id; to try another version of
  Zoom, Minimap, Recipes or SmartF3, the fixed copy would have to be removed from the
  build first.
