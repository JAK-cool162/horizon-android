//! A GUI theme from a content mod (docs/mods.md): `assets/<mod id>/horizon/gui_theme.json` recolours
//! the game's GUI textures as they are packed into the atlas, so every menu, container and the HUD's
//! frames take its colours without the mod shipping (or the player needing) any of vanilla's art.
//!
//! The grey pixels of a GUI texture (saturation up to `max_saturation`) take the colour of the
//! `gradient` at their brightness, keeping their alpha; coloured ones (icons, hearts, the XP bar)
//! stay as they are. A resource pack's own GUI textures are recoloured too: they are packed the
//! same way. The labels vanilla draws in dark grey over containers take `label_color`.
use serde::Deserialize;
use std::path::Path;

/// Where a mod puts its theme, under `assets/<its namespace>/`.
pub const FILE: &str = "horizon/gui_theme.json";

/// The colour vanilla draws container labels in (`0x404040`, no shadow), as `hud::rgba` packs it.
pub const VANILLA_LABEL: u32 = crate::hud::rgba(64, 64, 64, 255);

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Colours from black to white, at least two, evenly spaced.
    stops: Vec<[f32; 3]>,
    max_saturation: f32,
    /// Containers' labels, `hud::rgba` packed; none keeps vanilla's.
    pub label_color: Option<u32>,
}

#[derive(Deserialize)]
struct File {
    gradient: Vec<String>,
    #[serde(default = "default_saturation")]
    max_saturation: f32,
    #[serde(default)]
    label_color: Option<String>,
}

fn default_saturation() -> f32 {
    0.12
}

/// `#rrggbb` as 0..1 channels.
fn hex(text: &str) -> Option<[f32; 3]> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some([16, 8, 0].map(|shift| ((value >> shift) & 0xff) as f32 / 255.0))
}

impl Theme {
    /// A theme file's text; `Err` says what is wrong with it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let stops = file
            .gradient
            .iter()
            .map(|c| hex(c).ok_or_else(|| format!("{c:?} is not a #rrggbb colour")))
            .collect::<Result<Vec<_>, _>>()?;
        if !(2..=16).contains(&stops.len()) {
            return Err("a gradient has 2 to 16 colours".into());
        }
        let label_color = match &file.label_color {
            Some(c) => {
                let [r, g, b] = hex(c).ok_or_else(|| format!("{c:?} is not a #rrggbb colour"))?;
                let byte = |v: f32| (v * 255.0).round() as u8;
                Some(crate::hud::rgba(byte(r), byte(g), byte(b), 255))
            }
            None => None,
        };
        Ok(Self {
            stops,
            max_saturation: file.max_saturation.clamp(0.0, 1.0),
            label_color,
        })
    }

    /// The first theme of the resource root's namespaces, in name order, that reads.
    pub fn load(root: &Path) -> Option<Self> {
        let mut namespaces: Vec<_> = std::fs::read_dir(root.join("assets"))
            .ok()?
            .flatten()
            .map(|e| e.path())
            .collect();
        namespaces.sort();
        namespaces.into_iter().find_map(|ns| {
            let text = std::fs::read_to_string(ns.join(FILE)).ok()?;
            Self::parse(&text)
                .map_err(|e| log::warn!("GUI theme {}: {e}", ns.join(FILE).display()))
                .ok()
        })
    }

    /// Whether the GUI texture `name` (`gui/sprites/widget/button`) takes the theme: vanilla's and
    /// packs' GUI, but not the title's logo or the animated backgrounds packs draw on purpose.
    pub fn applies_to(name: &str) -> bool {
        name.starts_with("gui/")
            && !name.starts_with("gui/title/")
            && !name.starts_with(crate::gui_backgrounds::DIR)
    }

    /// Recolours RGBA `pixels` in place.
    pub fn apply(&self, pixels: &mut [u8]) {
        let last = (self.stops.len() - 1) as f32;
        for p in pixels.chunks_exact_mut(4) {
            if p[3] == 0 {
                continue;
            }
            let [r, g, b] = [p[0], p[1], p[2]].map(|v| v as f32 / 255.0);
            let (max, min) = (r.max(g).max(b), r.min(g).min(b));
            let saturation = if max > 0.0 { (max - min) / max } else { 0.0 };
            if saturation > self.max_saturation {
                continue;
            }
            let at = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 1.0) * last;
            let i = (at.floor() as usize).min(self.stops.len() - 2);
            let t = at - i as f32;
            let (a, b) = (self.stops[i], self.stops[i + 1]);
            for c in 0..3 {
                p[c] = ((a[c] + (b[c] - a[c]) * t) * 255.0).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greys_take_the_gradient_and_colours_stay() {
        let theme =
            Theme::parse(r##"{"gradient": ["#000000", "#204080"], "label_color": "#ffffff"}"##)
                .unwrap();
        // White, mid grey, a red heart pixel, a transparent pixel.
        let mut pixels = [
            255, 255, 255, 255, //
            128, 128, 128, 200, //
            200, 30, 30, 255, //
            90, 90, 90, 0,
        ];
        theme.apply(&mut pixels);
        assert_eq!(&pixels[0..4], &[0x20, 0x40, 0x80, 255]);
        // Half way along, the alpha kept.
        assert_eq!(&pixels[4..8], &[16, 32, 64, 200]);
        assert_eq!(&pixels[8..12], &[200, 30, 30, 255]);
        assert_eq!(&pixels[12..16], &[90, 90, 90, 0]);
        assert_eq!(
            theme.label_color,
            Some(crate::hud::rgba(255, 255, 255, 255))
        );
    }

    #[test]
    fn bad_themes_say_why() {
        assert!(Theme::parse(r##"{"gradient": ["#000000"]}"##).is_err());
        assert!(Theme::parse(r##"{"gradient": ["#000000", "blue"]}"##).is_err());
        assert!(Theme::applies_to("gui/sprites/widget/button"));
        assert!(Theme::applies_to("gui/container/inventory"));
        assert!(!Theme::applies_to("gui/title/minecraft"));
        assert!(!Theme::applies_to("item/diamond"));
    }
}
