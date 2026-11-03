# Horizon Android 0.7.8 development

This version is in development; no device-wide performance improvement is established.

## Launcher layout

The launcher uses Material 3 surfaces, in the style of Zalith Launcher 2: a navigation
rail with icons on the left in landscape (a bottom bar in portrait), a top bar with an
account chip, tonal pill buttons, Material switches and card rows. **Play** shows a launch
panel beside the overview: the account, the version and profile, the mods enabled and a
**Launch** button; the installation's stages and progress stay on the same page. The pages
keep their actions, settings and keyboard/controller focus order.

**Controls**, beside **Launch**, starts the game on its title screen with the on-screen
controls editor open, so a layout can be arranged before joining a world (the saved server
is not joined). When a custom button's key is chosen, **Game action** lists the mods' keys
first, as `Mod: what it does`: the running `.hmod` keys and the native mods' keys. Such a
button presses the mod's key on whatever key it is bound to and reaches only the mods; it
stays in the layout while its mod is off.

The Mods, Resource packs and Shaders pages show each result's icon (fetched over HTTPS once
per session, at most 2 MiB, four at a time), where it comes from, and how it runs here with
the newest Minecraft version it lists: **Works**, **Partly** (an older version, or a Java
shader pack run through the translator) or **Doesn't run** (a Java mod). Tapping a result
opens its page: Install, its page on the source's site, the versions and loaders it lists, why
it runs only partly, and its description (Markdown and HTML shown as plain text, fetched from
the source when opened). Back, or the system Back gesture, returns to the list where it was.
The lists' rows at the top and bottom edges show their visible part while scrolling.

## Importing from another Horizon app

Setup > **Import from another Horizon app** opens Android's folder picker at the official
app's Files folder. The worlds, settings (options, graphics, touch and keypad layouts) and
mods, resource and shader packs it shows are copied into this app's profile. No world or
add-on is overwritten; replaced settings keep a `.before-import` copy. Accounts and the
downloaded Minecraft files are not exposed by any app, so they stay where they are.

## Importing Minecraft's files

**Import Minecraft files** (on Play before installing, and in Setup) reads a folder the player
picks, such as a Java launcher's folder or a copied `.minecraft`, finds the 26.3 client and
the asset objects, and puts those whose size and SHA-1 match Mojang's metadata in the
installer's cache. The installer verifies them again and downloads only the rest. The
installer's metadata lookup is shared with it (`official_install::wanted`).

## Native mods (.hnmod)

`.hnmod` native mods can be downloaded from the Horizon catalog, imported from a file
(Mods > Import file) or from an HTTPS link. Each is checked as the registry checks it and
kept in the profile's `horizon-native-mods/`, which Android Files shows as Native Mods.
Nothing in a `.hnmod` runs on the phone: a native mod plays only in an APK compiled with
it. Installed > Native Mods says which ones this APK holds (**Built in**), which need a
build, which the APK's fixed copy replaces (Zoom, Minimap, Recipes, SmartF3), and which no
APK can hold (another game version, newer hook points, desktop-only post-build steps or
modules).

**Build APK** in Installed > Native Mods builds an APK with them on the player's own GitHub
fork (its Android dev workflow, a fine-grained token in Setup), then downloads and installs
it. `tools/android_native_mods.py` builds an APK with native mods compiled in, starts your own
mod from a template (`new`) and packages it as a `.hnmod` (`pack`), and the Android
dev workflow takes a `native_mods` input of `.hnmod` links. See
[native mods on Android](android-native-mods.md).

## Mod API level 8

Sandboxed `.hmod` mods get `world.break-blocks` (break blocks with their drops) and the
optional `use-events.block-used` event (a player right-clicked a watched block). The new
example `right-harvest` replants ripe crops on right-click as a runtime mod, without an
APK build. See [mods that load without a rebuild](android-native-mods.md#api-level-8-more-of-the-game-without-a-rebuild).

## The mod bridge (API level 9)

Native mods can serve APIs of their own to sandboxed mods: capability `bridge` (`version`,
`get`, `call`, with `bridge_reply` and `bridge_event` back), and the hook points
`native_bridge_apis` and `native_bridge_call`. New APIs then come as shareable `.hnmod` files
instead of changes to the game's mod API. Native APIs can also ask a mod at once inside a
hook (`bridge-asks`, `native_bridge_ask`), so it can cancel a block break, an attack or a
command, as Fabric's callbacks do. Examples: the `extras_api` native mod (version 3: events,
cancellable callbacks, player helpers, and the window's video options for client mods) and the
`welcome-kit`, `spawn-guard` and `video-settings` mods that use it. See [the mod bridge](android-native-mods.md#the-mod-bridge-apis-that-native-mods-add).

## GIF backgrounds behind screens

A resource pack can put an animated GIF behind screens, a Horizon feature off unless a pack
gives one: `assets/minecraft/textures/gui/sprites/horizon/background/<screen>.gif`, where
`<screen>` is `menu` (menus), `inventory`, `creative`, `crafting_table`, `chest`, `furnace`,
`brewing_stand`, `crafter`, a workstation (`enchanting_table`, `anvil`, `grindstone`,
`smithing_table`, `stonecutter`, `cartography_table`, `loom`, `beacon`, `villager`), or
`container` for every container screen without its own. The pack overlay turns the GIF into
an animated strip and its `.png.mcmeta` (an animated PNG strip works as it is); the GUI atlas
keeps one frame's region and copies frames in as they turn, on ticks. The GIF covers the
screen, cropped to its shape, over the dark fill and under the panel. Frames up to
1024 x 512, about 48 MB and 240 frames at most (larger ones are scaled down, longer ones cut).

## A build with only its native mods

`HORIZON_ANDROID_BUNDLED_MODS=none` at build time leaves the fixed Android bundle (Zoom,
Minimap, Recipes, SmartF3) out of an APK: an APK with only the native mods built into it.

## Server menus

Text a server makes clickable with a `custom` click event (`ClickEvent.Custom`, since 1.21.6:
an id and an optional payload sent back to the server) was read as plain text, so a server
dialog's clickable lines (a kit list, tabs) did nothing. They now send
`ServerboundCustomClickActionPacket` from a dialog, the chat and books, as vanilla does. A tap on
a menu also no longer stays owned by a finger whose menu closed under it.

## Shader packs

The shader pack translator (`crates/horizon-shaders`) is upstream's of 2026-11-03: about 60
upstream fixes for packs as they are written (fog variables, interface blocks, anonymous
structs, image parameters, integer bounds, array initializers, the preprocessor). The renderer
is still this fork's, so a pack's `setup` and `begin` programs and its block outline program
don't run yet. On the packs read, MakeUp Ultra Fast no longer has 12 programs failing; BSL
and Complementary Reimagined translate as before. How they look and run on a phone is not
verified.

## Verification

- Android ARM64 Clippy with warnings denied passed.
- The content/storage harness (`tools/test_android_content.py`) and the Android Python
  tests passed, including the new `.hnmod` checks and the build tool's planning.
- The `new` template, with its two hooks applied by hand as horizon-mixins applies HEAD
  helpers, passed ARM64 Clippy with warnings denied.
- Not yet verified: the new layout on a device, and an APK built with a `.hnmod`
  (horizon-mixins applying a mod to this fork). Mods pinned to another Horizon commit may
  fail to apply.
- API level 8: the mod sandbox suite and the API reference test passed, Right Harvest's
  sandbox script passed (4 steps), and Clippy with warnings denied passed for
  horizon-mods, the SDK and the example mods (wasm32) and cargo-horizon-mod.
- The mod bridge: the mod sandbox suite (33 tests, including that `bridge` needs its
  capability), welcome-kit's three sandbox scripts (with the Extras API, without it, and with
  failing calls), ARM64 Clippy with warnings denied and a host check without code mods passed.
  Asks: spawn-guard's sandbox scripts passed, and Extras API 2 with all its hooks applied
  passed an ARM64 check. Not yet verified on a device: the Extras API in a running game.
