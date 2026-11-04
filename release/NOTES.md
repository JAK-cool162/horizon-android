Horizon Android 0.7.8 development build **1017** (arm64), from `android`. Installs over 1000 to 1016 (same key).

## New since 1013
- **Fog culling** (Sodium-style, same picture): sections fully hidden by fog aren't drawn where only the fog's colour is behind them (Nether, lava, powder snow, blindness). Desktop test, Nether at render distance 12: terrain quads 273,719 → 67,605, 4.8 → 12.6 FPS (software renderer, not phone FPS); the screenshots differ in no pixel.
- **RAM in the performance overlay**: the game's memory and the system's free memory.
- Performance recordings: terrain/world quads, fog-culled sections, shader pack pass times (GPU profile on).
- Launcher: result icons, source and compatibility badges, result pages with descriptions, a Controls button, smoother scrolling lists.
- Touch buttons can press mods' keys.
- Mod API level 10: block reads, durability, world outlines, mining as a player, GUI themes.

## Files
| File | What it is |
|---|---|
| `horizon-android-0.7.8-dev.1017-arm64-v8a.apk` | The app, Extras API 3.2 built in |
| `extras_api-3.2.0.hnmod` | Extras API source bundle (cave/entity/fog culling and overlay options) |
| `midnight_ui.hmod` | Dark navy GUI theme |
| `horizon-extras-api-guide.txt` | Writing mods for the Extras API |
| `SHA256SUMS.txt` | Checksums |

🤖 Generated with [Claude Code](https://claude.com/claude-code)
