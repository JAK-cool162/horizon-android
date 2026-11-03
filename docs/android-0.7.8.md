# Horizon Android 0.7.8 development

This version is in development; no device-wide performance improvement is established.

## Launcher layout

The launcher uses Material 3 surfaces, in the style of Zalith Launcher 2: a navigation
rail with icons on the left in landscape (a bottom bar in portrait), a top bar with an
account chip, tonal pill buttons, Material switches and card rows. **Play** shows a launch
panel beside the overview: the account, the version and profile, the mods enabled and a
**Launch** button; the installation's stages and progress stay on the same page. The pages
keep their actions, settings and keyboard/controller focus order.

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
