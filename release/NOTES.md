Horizon Android 0.7.8 development build **1018** (arm64). Installs over 1000 to 1017 (same key).

## New since 1013
- **Hapi, the Horizon API** (the Extras API renamed, 3.3.0): bridge name `hapi`; old mods calling `extras` keep working. New options: fog/entity/cave culling and the FPS & RAM overlay.
- **Fog culling** (Sodium-style, same picture): chunks hidden by fog aren't drawn in the Nether, lava, powder snow, blindness. Desktop test (Nether, render distance 12, software renderer — not phone FPS): 273,719 → 67,605 terrain quads, 4.8 → 12.6 FPS, screenshots identical.
- **RAM in the performance overlay** (Video Settings > Horizon > Overlay, or Video Settings+).
- Launcher: result icons, source/compatibility badges, result pages, a Controls button, smoother lists. Touch buttons can press mods' keys.
- Mod API level 10: block reads, durability, world outlines, mining as a player, GUI themes.

## Files
| File | What it is |
|---|---|
| `horizon-android-0.7.8-dev.1018-arm64-v8a.apk` | The app, Hapi 3.3 built in |
| `hapi-3.3.0.hnmod` | Hapi's source bundle |
| `horizon-hapi-guide.txt` | How to write mods with Hapi and the SDK (also for AI assistants) |
| `liteminer.hmod` + `liteminer_client.hmod` | LiteMiner H 0.3: hold Left Alt to vein mine, outlines, cost panel, scroll for shapes |
| `liteminer-0.3.0-source.zip` | LiteMiner's source |
| `video_settings-0.2.0.hmod` | Video Settings+: presets, every option, culling and the overlay |
| `midnight_ui.hmod` | Dark navy GUI theme |
| `SHA256SUMS.txt` | Checksums |

🤖 Generated with [Claude Code](https://claude.com/claude-code)
