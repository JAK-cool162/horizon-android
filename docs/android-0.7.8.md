# Horizon Android 0.7.8 development

This version is in development; no device-wide performance improvement is established.

## Launcher layout

The launcher uses Material 3 surfaces, in the style of Zalith Launcher 2: a navigation
rail with icons on the left in landscape (a bottom bar in portrait), a top bar with an
account chip, tonal pill buttons, Material switches and card rows. **Play** shows a launch
panel beside the overview: the account, the version and profile, the mods enabled and a
**Launch** button; the installation's stages and progress stay on the same page. The pages
keep their actions, settings and keyboard/controller focus order.

## Native mods (.hnmod)

`.hnmod` native mods can be downloaded from the Horizon catalog, imported from a file
(Mods > Import file) or from an HTTPS link. Each is checked as the registry checks it and
kept in the profile's `horizon-native-mods/`, which Android Files shows as Native Mods.
Nothing in a `.hnmod` runs on the phone: a native mod plays only in an APK compiled with
it. Installed > Native Mods says which ones this APK holds (**Built in**), which need a
build, which the APK's fixed copy replaces (Zoom, Minimap, Recipes, SmartF3), and which no
APK can hold (another game version, newer hook points, desktop-only post-build steps or
modules).

`tools/android_native_mods.py` builds an APK with native mods compiled in, starts your own
mod from a template (`new`) and packages it as a `.hnmod` (`pack`), and the Android
dev workflow takes a `native_mods` input of `.hnmod` links. See
[native mods on Android](android-native-mods.md).

## Verification

- Android ARM64 Clippy with warnings denied passed.
- The content/storage harness (`tools/test_android_content.py`) and the Android Python
  tests passed, including the new `.hnmod` checks and the build tool's planning.
- The `new` template, with its two hooks applied by hand as horizon-mixins applies HEAD
  helpers, passed ARM64 Clippy with warnings denied.
- Not yet verified: the new layout on a device, and an APK built with a `.hnmod`
  (horizon-mixins applying a mod to this fork). Mods pinned to another Horizon commit may
  fail to apply.
