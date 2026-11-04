Horizon Android 0.7.8 development build **1016** (arm64), from [`9114629fe`](https://github.com/JAK-cool162/horizon-android/commit/9114629fe) on `android`.

> A development build. Report what works and what doesn't.

## Install

- **`horizon-android-0.7.8-dev.1016-arm64-v8a.apk`**: the app (64-bit phones). Package `dev.horizon.android.dev`: it installs beside a stable Horizon Android, and updates dev builds 1000 to 1015 signed with the same key.
- Mods: launcher **Mods > Import file**, then turn them on under **Installed**.

## New since 1013

**Launcher**
- Mods, Resource packs and Shaders show each result's **icon**, its **source**, and how it runs here with the newest version it lists: **Works**, **Partly** or **Doesn't run**.
- **Tap a result** to open its page: Install, Open page, versions, loaders, and its full description.
- **Controls**, beside Launch: opens the on-screen controls editor before you play.
- Lists no longer hide a row (and its buttons) before it has scrolled out.

**Touch controls**
- A custom button can press a **mod's key**: the mods' keys are listed first under *Game action*.

**Mod API level 10**
- Client mods: `game.get-block`, `input.held-durability`, `render.set-outlines` (block outlines in the world). They may also run their server half's commands.
- Server mods: `world.mine-blocks` mines as the player would: tool loot, durability, hunger and protection apply.
- **GUI themes**: a content `.hmod` can recolour every menu and container with `assets/<id>/horizon/gui_theme.json`.

## Files

| File | What it is |
|---|---|
| `horizon-android-0.7.8-dev.1016-arm64-v8a.apk` | The app, with Extras API 3.1 built in |
| `extras_api-3.1.0.hnmod` | The Extras API native mod's source bundle (already in this APK) |
| `horizon-extras-api-guide.txt` | How to write mods for the Extras API |
| `midnight_ui.hmod` | A dark navy GUI theme. Takes effect on restart or F3+T. |
| `SHA256SUMS.txt` | Checksums |

The example mods of 1013 (Video Settings+, Spawn Guard, Welcome Kit, Right Harvest) still work: get them from the [1013 release](https://github.com/JAK-cool162/horizon-android/releases/tag/v0.7.8-dev.1013).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
