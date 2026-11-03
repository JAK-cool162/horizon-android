//! Example Horizon mod: the frame rate in the top-right corner. It is a sandboxed
//! WebAssembly `.hmod`, so the game loads it at runtime: import it and turn it on, with no
//! rebuild. `/fps` hides or shows the panel, `/fps lows` adds the slowest frame and the 1% low.
use horizon_mod_sdk::{
    Mod, Registration,
    hud::{self, Corner, Line},
    render,
};

struct FpsCounter {
    shown: bool,
    lows: bool,
    /// The panel last sent, so an unchanged one costs no call.
    last: Vec<String>,
}

/// The panel's lines for frame rate `fps`, slowest frame `min` and 1% low `low`.
fn lines(fps: u32, min: u32, low: u32, lows: bool) -> Vec<String> {
    let mut lines = vec![format!("{fps} FPS")];
    if lows {
        lines.push(format!("min {min}  1% {low}"));
    }
    lines
}

impl Mod for FpsCounter {
    fn init() -> Result<(Self, Registration), String> {
        let hooks = Registration::new().command("fps").on_tick();
        Ok((
            FpsCounter {
                shown: true,
                lows: false,
                last: Vec::new(),
            },
            hooks,
        ))
    }

    fn tick(&mut self, _tick: u64) {
        let now = if self.shown {
            let rate = render::frame_stats();
            lines(rate.fps, rate.min, rate.low, self.lows)
        } else {
            Vec::new()
        };
        if now == self.last {
            return;
        }
        if now.is_empty() {
            hud::clear();
        } else {
            let panel: Vec<Line> = now
                .iter()
                .map(|text| Line {
                    text: text.clone(),
                    color: 0xFFFFFF,
                })
                .collect();
            hud::set_panel(Corner::TopRight, &panel);
        }
        self.last = now;
    }

    fn command(&mut self, _name: &str, args: &str) -> Result<Option<String>, String> {
        Ok(Some(match args.trim() {
            "lows" => {
                self.lows = !self.lows;
                format!(
                    "Slowest frame and 1% low {}",
                    if self.lows { "shown" } else { "hidden" }
                )
            }
            "" => {
                self.shown = !self.shown;
                format!(
                    "FPS counter {}",
                    if self.shown { "shown" } else { "hidden" }
                )
            }
            _ => return Err("usage: /fps [lows]".into()),
        }))
    }
}

horizon_mod_sdk::export_mod!(FpsCounter);
