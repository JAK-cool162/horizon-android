//! Example Horizon client mod on the mod bridge: a Sodium-style video settings screen. The
//! screen is this sandboxed mod's (capability `ui`); the Extras API native mod
//! (`extras_api.hnmod`, version 3) reads and applies the window's options (`options`,
//! `set_option`, `shader_packs`), since a sandboxed mod cannot change them itself.
use horizon_mod_sdk::ui::{self, ButtonWidget, Screen, TextWidget, Widget};
use horizon_mod_sdk::{Mod, Registration, bridge, export_mod};
use serde_json::{Value, json};

const API: &str = "extras";
const VERSION: u32 = 3;

/// An option the screen cycles: its name in the API, its label, and its values with their text.
struct Choice {
    name: &'static str,
    label: &'static str,
    values: &'static [(&'static str, &'static str)],
}

/// Values as JSON text: numbers, strings in quotes, booleans.
const CHOICES: &[Choice] = &[
    Choice {
        name: "render_distance",
        label: "Render Distance",
        values: &[
            ("2", "2"),
            ("4", "4"),
            ("6", "6"),
            ("8", "8"),
            ("10", "10"),
            ("12", "12"),
            ("16", "16"),
            ("24", "24"),
            ("32", "32"),
        ],
    },
    Choice {
        name: "simulation_distance",
        label: "Simulation Distance",
        values: &[
            ("5", "5"),
            ("6", "6"),
            ("8", "8"),
            ("10", "10"),
            ("12", "12"),
            ("16", "16"),
        ],
    },
    Choice {
        name: "max_fps",
        label: "Max Framerate",
        values: &[
            ("30", "30"),
            ("45", "45"),
            ("60", "60"),
            ("90", "90"),
            ("120", "120"),
            ("260", "Unlimited"),
        ],
    },
    Choice {
        name: "graphics",
        label: "Graphics",
        values: &[
            ("\"fast\"", "Fast"),
            ("\"fancy\"", "Fancy"),
            ("\"fabulous\"", "Fabulous"),
        ],
    },
    Choice {
        name: "clouds",
        label: "Clouds",
        values: &[
            ("\"false\"", "Off"),
            ("\"fast\"", "Fast"),
            ("\"true\"", "Fancy"),
        ],
    },
    Choice {
        name: "particles",
        label: "Particles",
        values: &[("0", "All"), ("1", "Decreased"), ("2", "Minimal")],
    },
    Choice {
        name: "smooth_lighting",
        label: "Smooth Lighting",
        values: &[("false", "Off"), ("true", "On")],
    },
    Choice {
        name: "entity_shadows",
        label: "Entity Shadows",
        values: &[("false", "Off"), ("true", "On")],
    },
    Choice {
        name: "biome_blend",
        label: "Biome Blend",
        values: &[
            ("0", "Off"),
            ("1", "3x3"),
            ("2", "5x5"),
            ("3", "7x7"),
            ("5", "11x11"),
        ],
    },
    Choice {
        name: "mipmap_levels",
        label: "Mipmap Levels",
        values: &[("0", "Off"), ("1", "1"), ("2", "2"), ("4", "4")],
    },
    Choice {
        name: "gamma",
        label: "Brightness",
        values: &[
            ("0.0", "Moody"),
            ("0.5", "Default"),
            ("1.0", "Bright"),
            ("5.0", "Fullbright"),
        ],
    },
];

/// Horizon's culling, Sodium's kind: less drawn, the same picture (Extras API 3.2). Fog culling
/// skips what the Nether's fog, lava, powder snow, blindness or darkness hide entirely; entity
/// culling skips entities and block entities behind opaque blocks; cave culling skips sections
/// no open face leads to (vanilla's occlusion graph). The overlay shows frame times and memory.
const PERFORMANCE: &[Choice] = &[
    Choice {
        name: "fog_culling",
        label: "Fog Culling",
        values: &[("false", "Off"), ("true", "On")],
    },
    Choice {
        name: "entity_culling",
        label: "Entity Culling",
        values: &[("false", "Off"), ("true", "On")],
    },
    Choice {
        name: "cave_culling",
        label: "Cave Culling",
        values: &[("false", "Off"), ("true", "On")],
    },
    Choice {
        name: "overlay",
        label: "FPS & RAM Overlay",
        values: &[
            ("\"off\"", "Off"),
            ("\"fps\"", "Numbers"),
            ("\"graph\"", "Numbers + Graph"),
        ],
    },
];

/// The one-tap presets: their options in order (the graphics preset first, as it sets others).
const PRESETS: [(&str, &[(&str, &str)]); 3] = [
    (
        "Potato",
        &[
            ("graphics", "\"fast\""),
            ("render_distance", "6"),
            ("simulation_distance", "5"),
            ("max_fps", "30"),
            ("clouds", "\"false\""),
            ("particles", "2"),
            ("smooth_lighting", "false"),
            ("entity_shadows", "false"),
            ("biome_blend", "0"),
            ("mipmap_levels", "0"),
        ],
    ),
    (
        "Balanced",
        &[
            ("graphics", "\"fast\""),
            ("render_distance", "8"),
            ("simulation_distance", "6"),
            ("max_fps", "60"),
            ("clouds", "\"fast\""),
            ("particles", "1"),
        ],
    ),
    (
        "Quality",
        &[
            ("graphics", "\"fancy\""),
            ("render_distance", "12"),
            ("simulation_distance", "8"),
            ("max_fps", "60"),
        ],
    ),
];

/// Widget ids: presets from 10, choices from 100, the shader pack 200, performance from 300.
const PRESET_ID: u32 = 10;
const CHOICE_ID: u32 = 100;
const SHADER_ID: u32 = 200;
const PERFORMANCE_ID: u32 = 300;

/// The value after `current` in `choice`, as JSON (the first when `current` is none of them).
fn next(choice: &Choice, current: &Value) -> Value {
    let at = choice
        .values
        .iter()
        .position(|(v, _)| serde_json::from_str::<Value>(v).is_ok_and(|v| same(&v, current)));
    let (v, _) = choice.values[at.map_or(0, |i| (i + 1) % choice.values.len())];
    serde_json::from_str(v).unwrap_or(Value::Null)
}

/// Equal values, numbers by value (`8` and `8.0`).
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
        _ => a == b,
    }
}

/// `Label: Value` for the option's current value.
fn label(choice: &Choice, current: &Value) -> String {
    let text = choice
        .values
        .iter()
        .find(|(v, _)| serde_json::from_str::<Value>(v).is_ok_and(|v| same(&v, current)))
        .map(|(_, t)| (*t).to_owned())
        .unwrap_or_else(|| match current {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        });
    format!("{}: {text}", choice.label)
}

/// The `set_option` arguments a preset sends, in order.
fn preset_calls(preset: usize) -> Vec<String> {
    PRESETS[preset]
        .1
        .iter()
        .map(|(name, value)| format!(r#"{{"name":"{name}","value":{value}}}"#))
        .collect()
}

#[derive(Default)]
struct VideoSettings {
    /// The options as the native API last answered, and its shader packs.
    options: Option<Value>,
    packs: Vec<(String, String)>,
    /// The mod's screen is up.
    open: bool,
    /// What went wrong last, shown on the page.
    error: Option<String>,
}

impl VideoSettings {
    fn widgets(&self) -> Vec<Widget> {
        let text = |id, text: &str, color| {
            Widget::Text(TextWidget {
                id,
                text: text.into(),
                color,
            })
        };
        let button = |id, label: String| Widget::Button(ButtonWidget { id, label });
        if bridge::version(API).is_none_or(|v| v < VERSION) {
            return vec![text(
                1,
                "Needs the Extras API 3 native mod (extras_api.hnmod)",
                0xFF5555,
            )];
        }
        let Some(options) = &self.options else {
            return vec![text(1, "Reading the options...", 0xAAAAAA)];
        };
        let mut widgets = vec![text(1, "Presets", 0xFFFF55)];
        for (i, (name, _)) in PRESETS.iter().enumerate() {
            widgets.push(button(PRESET_ID + i as u32, format!("Preset: {name}")));
        }
        widgets.push(text(2, "Options", 0xFFFF55));
        for (i, choice) in CHOICES.iter().enumerate() {
            widgets.push(button(
                CHOICE_ID + i as u32,
                label(choice, &options[choice.name]),
            ));
        }
        let pack = options["shader_pack"].as_str().unwrap_or_default();
        let title = self
            .packs
            .iter()
            .find(|(f, _)| f == pack)
            .map_or(pack, |(_, t)| t.as_str());
        let shown = if pack.is_empty() { "Off" } else { title };
        widgets.push(button(SHADER_ID, format!("Shader Pack: {shown}")));
        // An Extras API before 3.2 doesn't answer these: no rows for them.
        let performance: Vec<(usize, &Choice)> = PERFORMANCE
            .iter()
            .enumerate()
            .filter(|(_, c)| !options[c.name].is_null())
            .collect();
        if !performance.is_empty() {
            widgets.push(text(4, "Performance", 0xFFFF55));
            for (i, choice) in performance {
                widgets.push(button(
                    PERFORMANCE_ID + i as u32,
                    label(choice, &options[choice.name]),
                ));
            }
        }
        if let Some(error) = &self.error {
            widgets.push(text(3, error, 0xFF5555));
        }
        widgets
    }

    fn show(&self) {
        let widgets = self.widgets();
        ui::set_settings(&widgets);
        if self.open {
            ui::open_screen(&Screen {
                title: "Video Settings+".into(),
                widgets,
            });
        }
    }

    fn set(&mut self, args: String) {
        if let Err(e) = bridge::call(API, "set_option", &args) {
            self.error = Some(e);
        }
    }
}

impl Mod for VideoSettings {
    fn init() -> Result<(Self, Registration), String> {
        let _ = bridge::call(API, "options", "");
        let _ = bridge::call(API, "shader_packs", "");
        let me = VideoSettings::default();
        me.show();
        Ok((me, Registration::new().command("video")))
    }

    fn command(&mut self, _name: &str, _args: &str) -> Result<Option<String>, String> {
        self.open = true;
        self.show();
        Ok(None)
    }

    fn ui_settings_opened(&mut self) {
        let _ = bridge::call(API, "options", "");
        self.show();
    }

    fn ui_closed(&mut self) {
        self.open = false;
    }

    fn ui_select(&mut self, id: u32, _row: u32) {
        let Some(options) = &self.options else {
            return;
        };
        self.error = None;
        if (PRESET_ID..PRESET_ID + PRESETS.len() as u32).contains(&id) {
            for args in preset_calls((id - PRESET_ID) as usize) {
                self.set(args);
            }
        } else if let Some(choice) = CHOICES
            .get(id.wrapping_sub(CHOICE_ID) as usize)
            .or_else(|| PERFORMANCE.get(id.wrapping_sub(PERFORMANCE_ID) as usize))
        {
            let value = next(choice, &options[choice.name]);
            self.set(json!({"name": choice.name, "value": value}).to_string());
        } else if id == SHADER_ID {
            let current = options["shader_pack"].as_str().unwrap_or_default();
            let files: Vec<&str> = std::iter::once("")
                .chain(self.packs.iter().map(|(f, _)| f.as_str()))
                .collect();
            let at = files.iter().position(|f| *f == current).unwrap_or(0);
            let pick = files[(at + 1) % files.len()];
            self.set(json!({"name": "shader_pack", "value": pick}).to_string());
        }
    }

    fn bridge_reply(&mut self, _id: u64, api: &str, function: &str, answer: Result<&str, &str>) {
        if api != API {
            return;
        }
        match (function, answer) {
            ("options" | "set_option", Ok(json)) => {
                self.options = serde_json::from_str(json).ok();
            }
            ("shader_packs", Ok(json)) => {
                let packs: Vec<Value> = serde_json::from_str(json).unwrap_or_default();
                self.packs = packs
                    .iter()
                    .map(|p| {
                        let file = p["file"].as_str().unwrap_or_default().to_owned();
                        let title = p["title"].as_str().unwrap_or(&file).to_owned();
                        (file, title)
                    })
                    .collect();
            }
            (_, Err(why)) => self.error = Some(why.to_owned()),
            _ => {}
        }
        self.show();
    }
}

export_mod!(VideoSettings);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_cycle_from_the_current_value_and_wrap() {
        let render = &CHOICES[0];
        assert_eq!(next(render, &json!(8)), json!(10));
        assert_eq!(next(render, &json!(32)), json!(2));
        // A value the list doesn't have starts it over.
        assert_eq!(next(render, &json!(7)), json!(2));
        let clouds = &CHOICES[4];
        assert_eq!(next(clouds, &json!("fast")), json!("true"));
        assert_eq!(label(clouds, &json!("true")), "Clouds: Fancy");
        let brightness = &CHOICES[10];
        assert_eq!(label(brightness, &json!(5.0)), "Brightness: Fullbright");
        assert_eq!(label(brightness, &json!(0.7)), "Brightness: 0.7");
    }
    #[test]
    fn presets_set_the_graphics_preset_first() {
        let potato = preset_calls(0);
        assert_eq!(potato[0], r#"{"name":"graphics","value":"fast"}"#);
        assert!(potato.contains(&r#"{"name":"max_fps","value":30}"#.to_owned()));
        for call in potato
            .iter()
            .chain(&preset_calls(1))
            .chain(&preset_calls(2))
        {
            assert!(serde_json::from_str::<Value>(call).is_ok(), "{call}");
        }
    }
}
