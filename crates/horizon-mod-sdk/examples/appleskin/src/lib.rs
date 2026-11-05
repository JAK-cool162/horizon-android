//! AppleSkin H: AppleSkin's food HUD (squeek502, Unlicense) on Horizon's hunger bar, drawn with
//! the mod API's HUD shapes over the vanilla bar:
//! - a gold outline over each drumstick your saturation covers,
//! - the drumsticks (and saturation) the food in your hand would add, flashing,
//! - a light shade across the bar for exhaustion, where the game runs the world (a client is
//!   never sent it: on a server and single player's server thread it isn't shown).
use horizon_mod_sdk::{
    Mod, Registration, game, input,
    render::{self, FillShape, Rect, Shape, SpriteShape},
};

/// The saturation outline's gold, and the exhaustion shade.
const GOLD: u32 = 0xFF_E8_B8_28;
const EXHAUSTION: u32 = 0x50_FF_FF_FF;

#[derive(Default)]
struct AppleSkin {
    /// The shapes last sent (their `Debug` form): an unchanged HUD costs no call.
    last: String,
}

/// Where vanilla draws the hunger bar's drumstick `n` (0 rightmost, 9 leftmost) on a `w` × `h`
/// GUI: `Gui.extractFood`, the right of the hotbar, 39 pixels from the bottom.
fn icon(w: f32, h: f32, n: u32) -> (f32, f32) {
    let right = (w / 2.0).floor() + 91.0;
    (right - n as f32 * 8.0 - 9.0, h - 39.0)
}

fn fill(x: f32, y: f32, w: f32, h: f32, color: u32) -> Shape {
    Shape::Fill(FillShape {
        rect: Rect { x, y, w, h },
        color,
    })
}

/// The gold outline over drumstick `(x, y)` for `share` of its two points (0 to 2): the whole
/// border for a full drumstick, else its sides in turn (top, right, bottom, left) as far as the
/// share goes, as AppleSkin's quarter outlines.
fn outline(out: &mut Vec<Shape>, (x, y): (f32, f32), share: f32, color: u32) {
    let sides = ((share / 2.0) * 4.0).ceil().clamp(0.0, 4.0) as u32;
    let rects = [
        (x, y, 9.0, 1.0),
        (x + 8.0, y, 1.0, 9.0),
        (x, y + 8.0, 9.0, 1.0),
        (x, y, 1.0, 9.0),
    ];
    for &(rx, ry, rw, rh) in rects.iter().take(sides as usize) {
        out.push(fill(rx, ry, rw, rh, color));
    }
}

/// The flash's opacity at `tick`: up and down once a second, never quite gone.
fn flash(tick: u64) -> u32 {
    let phase = (tick % 20) as f32 / 20.0;
    let a = 0.3 + 0.7 * (std::f32::consts::PI * phase).sin();
    ((a.clamp(0.0, 1.0) * 255.0) as u32) << 24 | 0x00FF_FFFF
}

/// Everything AppleSkin draws for this food, held food and screen.
fn shapes(food: &game::FoodStats, held: Option<input::FoodValue>, (w, h): (f32, f32), tick: u64) -> Vec<Shape> {
    let mut out = Vec::new();
    if !food.shown {
        return out;
    }
    // Exhaustion: a shade from the right, the share of 4 it has reached.
    if let Some(exhaustion) = food.exhaustion {
        let width = (exhaustion / 4.0).clamp(0.0, 1.0) * 81.0;
        if width >= 1.0 {
            let (x, y) = icon(w, h, 0);
            out.push(fill(x + 9.0 - width, y, width, 9.0, EXHAUSTION));
        }
    }
    let saturation = food.saturation.clamp(0.0, food.food as f32);
    for n in 0..10 {
        let share = (saturation - n as f32 * 2.0).clamp(0.0, 2.0);
        if share > 0.0 {
            outline(&mut out, icon(w, h, n), share, GOLD);
        }
    }
    // The food in hand: what it would add, flashing.
    if let Some(value) = held.filter(|v| v.nutrition > 0 && food.food < 20) {
        let tint = flash(tick);
        let after = (food.food + value.nutrition).min(20);
        for n in 0..10 {
            let (low, high) = (n * 2 + 1, n * 2 + 2);
            let sprite = if after >= high && food.food < high {
                "hud/food_full"
            } else if after == low && food.food < low {
                "hud/food_half"
            } else {
                continue;
            };
            let (x, y) = icon(w, h, n);
            out.push(Shape::Sprite(SpriteShape {
                sprite: sprite.into(),
                rect: Rect { x, y, w: 9.0, h: 9.0 },
                color: tint,
            }));
        }
        // And the saturation it would add, outlined in the flash.
        let gained = (saturation + value.saturation).min(after as f32);
        let gold = (tint & 0xFF00_0000) | (GOLD & 0x00FF_FFFF);
        for n in 0..10 {
            let from = (saturation - n as f32 * 2.0).clamp(0.0, 2.0);
            let to = (gained - n as f32 * 2.0).clamp(0.0, 2.0);
            if to > from {
                outline(&mut out, icon(w, h, n), to, gold);
            }
        }
    }
    out
}

impl Mod for AppleSkin {
    fn init() -> Result<(Self, Registration), String> {
        Ok((AppleSkin::default(), Registration::new().on_tick()))
    }

    fn tick(&mut self, tick: u64) {
        let now = match game::get_food() {
            Some(food) => shapes(&food, input::held_food(), render::gui_size(), tick),
            None => Vec::new(),
        };
        let key = format!("{now:?}");
        if key != self.last {
            render::set_hud(&now);
            self.last = key;
        }
    }
}

horizon_mod_sdk::export_mod!(AppleSkin);

#[cfg(test)]
mod tests {
    use super::*;

    fn food(food: u32, saturation: f32) -> game::FoodStats {
        game::FoodStats {
            food,
            saturation,
            exhaustion: None,
            shown: true,
        }
    }

    #[test]
    fn the_bar_sits_where_vanilla_draws_it() {
        // A 427 × 240 GUI: the right of the hotbar at 213 + 91, 39 from the bottom.
        assert_eq!(icon(427.0, 240.0, 0), (295.0, 201.0));
        assert_eq!(icon(427.0, 240.0, 9), (223.0, 201.0));
    }

    #[test]
    fn saturation_outlines_whole_and_part_drumsticks() {
        // 5 saturation: two whole drumsticks (4 sides each) and half of the third (2 sides).
        let drawn = shapes(&food(13, 5.0), None, (427.0, 240.0), 0);
        assert_eq!(drawn.len(), 10);
        // Saturation never shows past the food level.
        assert_eq!(shapes(&food(2, 20.0), None, (427.0, 240.0), 0).len(), 4);
    }

    #[test]
    fn held_food_flashes_the_drumsticks_it_adds() {
        // Bread (5 food, 6 saturation) at 13 food: drumsticks 7 to 9 (14 to 18) flash full.
        let bread = input::FoodValue { nutrition: 5, saturation: 6.0 };
        let sprites: Vec<String> = shapes(&food(13, 5.0), Some(bread), (427.0, 240.0), 5)
            .into_iter()
            .filter_map(|s| match s {
                Shape::Sprite(s) => Some(s.sprite),
                _ => None,
            })
            .collect();
        assert_eq!(sprites, ["hud/food_full"; 3]);
        // A full bar: nothing to add.
        assert!(shapes(&food(20, 5.0), Some(bread), (427.0, 240.0), 5)
            .iter()
            .all(|s| matches!(s, Shape::Fill(_))));
    }

    #[test]
    fn nothing_without_the_hunger_bar() {
        let mut stats = food(10, 5.0);
        stats.shown = false;
        assert!(shapes(&stats, None, (427.0, 240.0), 0).is_empty());
    }
}
