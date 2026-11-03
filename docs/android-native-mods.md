# Native mods (.hnmod) on Android

A `.hnmod` is a [native mod](native-mods.md): Rust source (a horizon-mixins bundle) that
changes the game before it is compiled. It is not a plugin, and a phone has no compiler,
so a native mod plays only in an APK built with it. Sandboxed `.hmod` mods still install
and run without a rebuild.

## Mods that load without a rebuild

Horizon Android also runs **sandboxed code mods**: a `.hmod` whose code is WebAssembly. The
game loads it at runtime, so there is no APK to build: Mods > **Import file**, then turn it
on in Installed (64-bit phones). One `.hmod` works on every phone and on desktop. It can do
what its declared capabilities allow, through the game's API
([`crates/horizon-mod-sdk/wit/horizon.wit`](../crates/horizon-mod-sdk/wit/horizon.wit)):
the HUD and its corners, frame statistics, zoom, keys and the use button, its own screen
and settings page, chat, commands, storage, recipes, and on servers players and blocks.
It cannot patch the game's code the way a `.hnmod` can.

On a computer, from this repository:

```sh
cargo run -p cargo-horizon-mod -- horizon-mod new my_mod --template code
cd my_mod
cargo run --manifest-path ../Cargo.toml -p cargo-horizon-mod -- horizon-mod test    # the sandbox runs tests/*.toml
cargo run --manifest-path ../Cargo.toml -p cargo-horizon-mod -- horizon-mod build   # target/horizon-mod/my_mod.hmod
```

[`crates/horizon-mod-sdk/examples/fps-counter`](../crates/horizon-mod-sdk/examples/fps-counter)
is such a mod: the frame rate in the top-right corner, `/fps` to hide it and `/fps lows` for
the slowest frame and the 1% low. Pick this kind of mod when the API covers it, and a
`.hnmod` when it has to change the game itself.

### API level 8: more of the game without a rebuild

API level 8 lets sandboxed mods do things that needed a `.hnmod` before:

| Function | What it does |
|---|---|
| `world::break_blocks(dimension, positions, drops)` (capability `world`) | Breaks up to 4096 blocks loaded in `dimension` as vanilla's `Level.destroyBlock` does: the break particles and sound, the loot when `drops` is true, and the block's fluid left in its place. |
| `ServerMod::block_used(player, dimension, pos, state, item)` (an optional event) | A player right-clicked a block the mod watches (`on_blocks`), holding `item`. It arrives on the next tick, after the game handled the click. |

[`crates/horizon-mod-sdk/examples/right-harvest`](../crates/horizon-mod-sdk/examples/right-harvest)
uses both: right-click a ripe crop (wheat, carrots, potatoes, beetroots, nether wart, cocoa)
and its loot drops and it is replanted, the way the RightHarvest native mod does, as one
`.hmod` that loads without building an APK. Its sandbox script (`tests/harvest.toml`) uses
the new `use` step and `drops` expectation. An APK with this runtime (level 8) is needed
once; after that such mods only need importing. An older APK does not load a mod that calls
`break_blocks` and says it needs a newer Horizon; older mods keep working on this one.

## The mod bridge: APIs that native mods add

Adding every function to the game's mod API by hand does not scale. With the **mod bridge**
(API level 9), a native mod defines an API of its own, and sandboxed `.hmod` mods call it by
name:

```
.hmod (WebAssembly, loads at runtime)  --bridge::call("extras", "command", ...)-->
    the game  -->  .hnmod (native, compiled in) serves "extras"  --reply-->  .hmod
```

- The `.hnmod` serves the API on two hook points: `native_bridge_apis` lists its name and
  version, and `native_bridge_call` answers calls. It can publish values
  (`native_bridge_publish`, read with `bridge::get`), send events to every mod or one
  (`native_bridge_event`, `native_bridge_event_to`), and **ask** a mod at once, inside one of
  its hooks, as Fabric's callbacks do (`native_bridge_ask`): the mod's answer can cancel what
  is happening. It runs as native code, so it can use anything the game can.
- The `.hmod` declares capability `bridge` (experimental: `experimental = ["bridge"]`); the
  player sees "use the APIs your native mods (.hnmod) add to the game" before turning it on.
  It checks `bridge::version("extras")` and calls `bridge::call(api, function, args)`.
  Arguments and answers are text in the form the API documents (JSON is common). The answer
  arrives as `bridge_reply(id, ...)` before the next tick's hooks, when the server's state is
  whole. Events arrive as `bridge_event`, and asks as `bridge_ask`, which returns `None` or an
  answer such as `Some("cancel")`. A mod checking `bridge::version` in `init` already sees
  the APIs.
- The `.hnmod` still has to be compiled into the APK once. Mods using its API do not:
  new `.hmod` mods for it only need importing. One shared `.hnmod` can serve any number of
  `.hmod` mods.

The examples:

| Example | What it is |
|---|---|
| [`examples/native/extras_api`](../crates/horizon-mod-sdk/examples/native/extras_api) | A `.hnmod` serving the API `extras` ([below](#the-extras-api)). A mod using it can do what a server operator can. |
| [`examples/welcome-kit`](../crates/horizon-mod-sdk/examples/welcome-kit) | A `.hmod` using it: each new player gets a starter kit once, and `/tps` shows the server's tick rate. Without the Extras API it tells the player which `.hnmod` to install. |
| [`examples/spawn-guard`](../crates/horizon-mod-sdk/examples/spawn-guard) | A `.hmod` on its asks: blocks within 16 of the Overworld's 0, 0 can't be broken or used by players below operator level 2, and explosions there break nothing. |

Package the native mod with `python3 tools/android_native_mods.py pack
crates/horizon-mod-sdk/examples/native/extras_api`. The bridge's hook points are not numbered
yet (`tools/native-hook-fragments/mod_bridge.json`), so `pack` pins the mod to the commit it
was packed on instead of writing `hooks`, and the in-app build applies it with
`--allow-version-mismatch`. A `.hmod`'s sandbox scripts can stand in for a native API:
`[bridge.extras]` with its `version`, `values` and `replies` (or `errors`) per function,
`bridge-event = { api, name, data }` and `bridge-ask = { api, name, data }` in a step,
`expect.calls`, and `expect.answer` (`""` for no answer). Its answers arrive within the step
(see welcome-kit's and spawn-guard's `tests/`).

The bridge runs only in the game it is called in: a server mod's calls are answered by the
server's native mods, a client mod's by the window's.

### The Extras API

Version 2 of `extras`, for server mods. Players are named by UUID (as `players::online` gives
them) or name; arguments and answers are JSON, except `command`'s line.

| Function | Arguments | What it does |
|---|---|---|
| `command` | a command line | Runs it as the server's console (permission level 4). |
| `subscribe`, `unsubscribe` | `["player_join", "break_block", ...]` | Starts or stops the events and asks below for this mod. |
| `players` | | The players online: `player`, `name`, `permission`, `dimension`, `x`, `y`, `z`, `game_mode`, `health`. |
| `tell`, `actionbar` | `{"player", "text"}` | A chat line, or a line over the hotbar. |
| `sidebar` | `{"player", "title", "lines": ["a", {"left", "right"}]}`; `"title": null` clears | A sidebar of up to 15 lines for that player. |
| `tablist` | `{"player", "header", "footer"}` | The tab list's header and footer. |
| `nickname` | `{"player", "name"}` (`null` clears) | The name others see. |
| `xp`, `levels` | `{"player", "points"}`, `{"player", "levels"}` | Experience (Mending first), or levels added or taken. |
| `sleep_ignored` | `{"player", "ignored"}` | Whether the night waits for them to sleep. |

Values (`bridge::get("extras", ...)`): `tps`, `mspt`.

| Event (after it happened) | Data |
|---|---|
| `player_join`, `player_leave` | `player`, `name` |
| `player_death` | `player`, `cause` (`death.attack.lava`, or null) |
| `mob_killed` | `player`, `entity`, `kind`: a mob their melee blow killed |
| `item_dropped` | `player`, `item`, `count` |

| Ask (answer `cancel`, or `cancel <reason>` told to the player) | Data |
|---|---|
| `break_block` | the player's fields, `x`, `y`, `z`, `block` (`minecraft:oak_log[axis=y]`) |
| `use_block` | the player's fields, `x`, `y`, `z`, `face`, `block`; once per hand |
| `attack_entity` | the player's fields, `entity`, `kind`, `at` |
| `pickup_item` | `player`, `item`, `count`; canceled, the stack waits 10 ticks |
| `command` | `player`, `name`, `command` (no slash); the reason is the command's answer |
| `explosion` | `dimension`, `x`, `y`, `z`, `blocks`; canceled, it breaks no block |

Asks are answered inside the game's tick, so they cost a call into each subscribed mod
whenever that thing happens; a mod subscribes only to what it needs.

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

## Building from the app

A phone cannot compile Rust, but your GitHub fork can. Installed > Native Mods starts with
**Build an APK with these mods**: **Build APK** uploads every native mod this APK can hold
(not the bundled copies or incompatible ones) to your fork's `native-mods` branch, starts the
**Android dev APKs** workflow with them, follows it, downloads the APK for this phone's ABI
into the profile's **APK builds** folder and opens Android's installer (**Install**). A build
takes about 40-70 minutes; **View** opens it on GitHub, and you can close Horizon meanwhile:
it picks the build up again when it opens.

Once: **Build APK** opens **Set up APK builds** until it has what it needs.

1. **Create a token on GitHub** opens GitHub's fine-grained token page (filled in where GitHub
   reads the parameters): only your fork, with **Contents**, **Actions** and **Secrets** read
   and write.
2. Copy the token on GitHub and tap **Paste the token**: it is kept encrypted with the Android
   Keystore and sent only to GitHub.
3. **Build APK**. The fork is the one this APK was built from, or Setup > **GitHub repository**.

The first build also makes the dev signing key: the app seals its token for the fork's
secrets (libsodium's sealed box, as GitHub requires), runs **Android dev signing key (one
time)**, which stores `ANDROID_DEV_KEYSTORE_BASE64`, and deletes that temporary secret.
Every later build is signed with the same key, so each one updates the last. A key made by
hand ([dev builds](android-dev-builds.md)) is used as it is.

The APK is a dev build (`dev.horizon.android.dev`): it installs beside a stable Horizon
Android, with its own worlds, and each later dev build updates it. Android asks once to let
Horizon install apps. The confirmation before a build says the mods are code with the app's
rights: build only mods you trust.

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

- No on-phone compiler and no dynamic loading of native code: changing the `.hnmod` mods
  means a new APK. Sandboxed `.hmod` mods do not, including those using a `.hnmod`'s API
  through the [mod bridge](#the-mod-bridge-apis-that-native-mods-add).
- A mod pinned to another Horizon commit may fail to apply or to compile on this fork.
- The fixed Android bundle wins over a `.hnmod` of the same id; to try another version of
  Zoom, Minimap, Recipes or SmartF3, the fixed copy would have to be removed from the
  build first.
