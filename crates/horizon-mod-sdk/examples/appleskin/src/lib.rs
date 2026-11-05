//! AppleSkin H: squeek502's AppleSkin (Unlicense) on Horizon's HUD, drawn as AppleSkin draws it,
//! with its own overlay art (public domain) over the vanilla bars:
//! - a gold outline along each drumstick your saturation fills, in quarters,
//! - the drumsticks, saturation and hearts the food in your hand would give, flashing,
//! - a dark checkered bar under the hunger bar for exhaustion (single player: no packet
//!   carries it, so not on a server).
//!
//! Not ported: item tooltips (the mod API has none yet) and the config screen.
use horizon_mod_sdk::{
    Mod, Registration, game, input,
    render::{self, FillShape, Rect, Shape, SpriteShape},
};

/// AppleSkin's saturation overlay (`icons.png`, row 0): a quarter, half, three quarters and a
/// whole drumstick's outline, one 9 × 9 icon each, and the colours of its letters.
const SATURATION: [[&str; 9]; 4] = [
    [
        ".........",
        ".........",
        ".........",
        ".........",
        "......c..",
        "......c..",
        ".....d.cc",
        "......d.c",
        "......dd.",
    ],
    [
        "...a.....",
        "....b....",
        ".....c...",
        "......c..",
        "......c..",
        "......c..",
        "....dd.cc",
        "......d.c",
        "......dd.",
    ],
    [
        "..aa.....",
        "....b....",
        ".....c...",
        "......c..",
        "......c..",
        "..d...c..",
        "...ddd.cc",
        "......d.c",
        "......dd.",
    ],
    [
        "..aa.....",
        ".b..b....",
        "c....c...",
        "c.....c..",
        ".d....c..",
        "..d...c..",
        "...ddd.cc",
        "......d.c",
        "......dd.",
    ],
];
const GOLD: [(u8, u32); 4] = [
    (b'a', 0xE9_D2_62),
    (b'b', 0xDD_C2_41),
    (b'c', 0xC2_A1_00),
    (b'd', 0x9F_86_09),
];
/// The exhaustion underlay's checkered pixels, at AppleSkin's 0.75 opacity.
const EXHAUSTION: u32 = 0xBF_28_28_28;
/// `maxHudOverlayFlashAlpha`'s default.
const MAX_FLASH: f32 = 0.65;
/// `FoodHelper`: the exhaustion that costs a point of saturation or food, and what a heal costs.
const MAX_EXHAUSTION: f32 = 4.0;
const REGEN_EXHAUSTION: f32 = 6.0;

/// The flash: up 0.125 a tick to 1.5, down to -0.5, shown clamped to 0 to 1.
struct Flash {
    unclamped: f32,
    up: bool,
}
impl Flash {
    fn tick(&mut self) {
        self.unclamped += if self.up { 0.125 } else { -0.125 };
        if self.unclamped >= 1.5 {
            self.up = false;
        } else if self.unclamped <= -0.5 {
            self.up = true;
        }
    }
    fn alpha(&self) -> f32 {
        self.unclamped.clamp(0.0, 1.0) * MAX_FLASH
    }
}

struct AppleSkin {
    flash: Flash,
    /// The shapes last sent (their `Debug` form), under and over the bars: an unchanged HUD
    /// costs no call.
    last: (String, String),
}

/// What AppleSkin draws: `under` the vanilla bars, and `over` them.
#[derive(Default, Debug)]
struct Hud {
    under: Vec<Shape>,
    over: Vec<Shape>,
}

fn fill(x: f32, y: f32, w: f32, h: f32, color: u32) -> Shape {
    Shape::Fill(FillShape {
        rect: Rect { x, y, w, h },
        color,
    })
}

fn sprite(out: &mut Vec<Shape>, name: &str, (x, y): (f32, f32), alpha: f32) {
    out.push(Shape::Sprite(SpriteShape {
        sprite: name.into(),
        rect: Rect {
            x,
            y,
            w: 9.0,
            h: 9.0,
        },
        color: argb(alpha, 0xFF_FF_FF),
    }));
}

fn argb(alpha: f32, rgb: u32) -> u32 {
    ((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24 | rgb
}

/// Where vanilla draws drumstick `i` (0 rightmost) on a `w` × `h` GUI: `Gui.extractFood`.
fn food_icon((w, h): (f32, f32), i: u32) -> (f32, f32) {
    let right = (w / 2.0).floor() + 91.0;
    (right - i as f32 * 8.0 - 9.0, h - 39.0)
}

/// Where vanilla draws heart `i` (0 leftmost, ten a row): `Gui.extractHearts`.
fn heart_icon((w, h): (f32, f32), i: u32, max_health: f32) -> (f32, f32) {
    let rows = (max_health / 2.0 / 10.0).ceil().max(1.0) as i32;
    let row_height = (10 - (rows - 2)).max(3) as f32;
    let left = (w / 2.0).floor() - 91.0;
    (
        left + (i % 10) as f32 * 8.0,
        h - 39.0 - (i / 10) as f32 * row_height,
    )
}

/// `drawSaturationOverlay`: the outline over drumsticks `start..` for `saturation` (of 20).
fn saturation_overlay(
    out: &mut Vec<Shape>,
    gui: (f32, f32),
    start: u32,
    saturation: f32,
    alpha: f32,
) {
    let end = (saturation / 2.0).ceil() as u32;
    for i in start..end.min(10) {
        let share = saturation / 2.0 - i as f32;
        let variant = if share >= 1.0 {
            3
        } else if share > 0.5 {
            2
        } else if share > 0.25 {
            1
        } else {
            0
        };
        let (x, y) = food_icon(gui, i);
        for (row, line) in SATURATION[variant].iter().enumerate() {
            // A run of one colour is one box.
            let bytes = line.as_bytes();
            let mut col = 0;
            while col < bytes.len() {
                let c = bytes[col];
                let run = bytes[col..].iter().take_while(|&&b| b == c).count();
                if let Some(&(_, rgb)) = GOLD.iter().find(|(k, _)| *k == c) {
                    let (px, py) = (x + col as f32, y + row as f32);
                    out.push(fill(px, py, run as f32, 1.0, argb(alpha, rgb)));
                }
                col += run;
            }
        }
    }
}

/// `FoodHelper.getEstimatedHealthIncrement(foodLevel, saturation, exhaustion)`: what natural
/// regeneration heals before the food runs below 18.
fn regen_health(mut food: u32, mut saturation: f32, mut exhaustion: f32) -> f32 {
    if !exhaustion.is_finite() || !saturation.is_finite() {
        return 0.0;
    }
    let above_max = f32::from_bits(MAX_EXHAUSTION.to_bits() + 1);
    let mut health = 0.0;
    while food >= 18 {
        while exhaustion > MAX_EXHAUSTION {
            exhaustion -= MAX_EXHAUSTION;
            if saturation > 0.0 {
                saturation = (saturation - 1.0).max(0.0);
            } else {
                food -= 1;
            }
        }
        if food >= 20 && saturation > f32::MIN_POSITIVE {
            // Fast regeneration, the iterations up to the next point of exhaustion at once.
            let limited = saturation.min(REGEN_EXHAUSTION);
            let iterations = ((above_max - exhaustion) / limited).ceil().max(1.0);
            health += limited / REGEN_EXHAUSTION * iterations;
            exhaustion += limited * iterations;
        } else if food >= 18 {
            health += 1.0;
            exhaustion += REGEN_EXHAUSTION;
        }
    }
    health
}

/// The health eating `value` would give back (`FoodHelper.getEstimatedHealthIncrement`), when
/// AppleSkin shows it (`shouldShowEstimatedHealth`).
fn health_gained(food: &game::FoodStats, value: &input::FoodValue) -> f32 {
    let shown = food.natural_regeneration && food.food < 18 && !food.health_effect;
    if !shown || food.health <= 0.0 || food.health >= food.max_health {
        return 0.0;
    }
    let level = (food.food + value.nutrition).min(20);
    let mut health = 0.0;
    if level >= 18 {
        let saturation = (food.saturation + value.saturation).min(level as f32);
        health = regen_health(level, saturation, food.exhaustion.unwrap_or(0.0));
    }
    if let Some((amplifier, ticks)) = value.regeneration {
        health += (ticks / (50u32 >> amplifier.min(31)).max(1)) as f32;
    }
    health
}

/// Everything AppleSkin draws for this food, held food and screen, `flash` the overlay's alpha.
fn hud(
    food: &game::FoodStats,
    held: Option<&input::FoodValue>,
    gui: (f32, f32),
    flash: f32,
) -> Hud {
    let mut hud = Hud::default();
    if !food.shown {
        return hud;
    }
    // drawExhaustionOverlay, under the bar: the right `ratio` of an 81-pixel checkerboard.
    if let Some(exhaustion) = food.exhaustion {
        let width = ((exhaustion / MAX_EXHAUSTION).clamp(0.0, 1.0) * 81.0) as u32;
        let (left, top) = food_icon(gui, 9);
        for row in 0..9u32 {
            for col in 81 - width..81 {
                if (col + row) % 2 == 1 {
                    let (x, y) = (left + col as f32, top + row as f32);
                    hud.under.push(fill(x, y, 1.0, 1.0, EXHAUSTION));
                }
            }
        }
    }
    let saturation = food.saturation.clamp(0.0, 20.0);
    saturation_overlay(&mut hud.over, gui, 0, saturation, 1.0);
    // The held food (FoodHelper.canConsume): what it would give, flashing.
    let Some(value) = held.filter(|v| food.food < 20 || v.always_edible) else {
        return hud;
    };
    // drawHealthOverlay: the hearts it heals.
    let gained = health_gained(food, value);
    let healed = (food.health + gained).min(food.max_health);
    if healed > food.health {
        let (full, half, container) = if food.hardcore {
            (
                "hud/heart/hardcore_full",
                "hud/heart/hardcore_half",
                "hud/heart/container_hardcore",
            )
        } else {
            ("hud/heart/full", "hud/heart/half", "hud/heart/container")
        };
        let start = (food.health.ceil() / 2.0).max(0.0) as u32;
        let end = (healed / 2.0).ceil().max(0.0) as u32;
        let last = healed.ceil() as u32;
        for i in start..end {
            let at = heart_icon(gui, i, food.max_health);
            sprite(&mut hud.over, container, at, flash * 0.25);
            let icon = if i * 2 + 1 == last { half } else { full };
            sprite(&mut hud.over, icon, at, flash);
        }
    }
    // drawHungerOverlay: the drumsticks it fills.
    let (full, half, container) = if value.hunger_effect {
        (
            "hud/food_full_hunger",
            "hud/food_half_hunger",
            "hud/food_empty_hunger",
        )
    } else {
        ("hud/food_full", "hud/food_half", "hud/food_empty")
    };
    let after = (food.food + value.nutrition).min(20);
    if value.nutrition > 0 {
        for i in food.food / 2..after.div_ceil(2) {
            let at = food_icon(gui, i);
            sprite(&mut hud.over, container, at, flash * 0.25);
            let icon = if i * 2 + 1 == after { half } else { full };
            sprite(&mut hud.over, icon, at, flash);
        }
    }
    // drawSaturationOverlay again, for what it adds.
    let new_food = (food.food + value.nutrition) as f32;
    let added = if saturation + value.saturation > new_food {
        new_food - saturation
    } else {
        value.saturation
    };
    if added != 0.0 {
        let total = (saturation + added).clamp(0.0, 20.0);
        let start = (saturation / 2.0).max(0.0) as u32;
        saturation_overlay(&mut hud.over, gui, start, total, flash);
    }
    hud
}

impl Mod for AppleSkin {
    fn init() -> Result<(Self, Registration), String> {
        let skin = AppleSkin {
            flash: Flash {
                unclamped: 0.0,
                up: true,
            },
            last: Default::default(),
        };
        Ok((skin, Registration::new().on_tick()))
    }

    fn tick(&mut self, _tick: u64) {
        self.flash.tick();
        let now = match game::get_food() {
            Some(food) => hud(
                &food,
                input::held_food().as_ref(),
                render::gui_size(),
                self.flash.alpha(),
            ),
            None => Hud::default(),
        };
        let keys = (format!("{:?}", now.under), format!("{:?}", now.over));
        if keys.0 != self.last.0 {
            render::set_hud_under(&now.under);
        }
        if keys.1 != self.last.1 {
            render::set_hud(&now.over);
        }
        self.last = keys;
    }
}

horizon_mod_sdk::export_mod!(AppleSkin);

#[cfg(test)]
mod tests {
    use super::*;

    const GUI: (f32, f32) = (427.0, 240.0);

    fn stats(food: u32, saturation: f32) -> game::FoodStats {
        game::FoodStats {
            food,
            saturation,
            exhaustion: None,
            shown: true,
            health: 10.0,
            max_health: 20.0,
            natural_regeneration: true,
            health_effect: false,
            hardcore: false,
        }
    }
    fn food(nutrition: u32, saturation: f32) -> input::FoodValue {
        input::FoodValue {
            nutrition,
            saturation,
            always_edible: false,
            hunger_effect: false,
            regeneration: None,
        }
    }
    fn sprites(shapes: &[Shape]) -> Vec<String> {
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Sprite(s) => Some(s.sprite.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_bars_sit_where_vanilla_draws_them() {
        // A 427 × 240 GUI: the hotbar's middle at 213, the bars 39 from the bottom.
        assert_eq!(food_icon(GUI, 0), (295.0, 201.0));
        assert_eq!(food_icon(GUI, 9), (223.0, 201.0));
        assert_eq!(heart_icon(GUI, 0, 20.0), (122.0, 201.0));
        assert_eq!(heart_icon(GUI, 9, 20.0), (194.0, 201.0));
        // 40 health: two rows, the second 10 higher.
        assert_eq!(heart_icon(GUI, 10, 40.0), (122.0, 191.0));
    }

    #[test]
    fn the_outline_follows_the_drumstick_in_quarters() {
        // Pixels of each variant in AppleSkin's icons.png: 9, 14, 17 and 21.
        let pixels = |saturation: f32| {
            let mut out = Vec::new();
            saturation_overlay(&mut out, GUI, 0, saturation, 1.0);
            out.iter()
                .map(|s| match s {
                    Shape::Fill(f) => f.rect.w as u32,
                    _ => 0,
                })
                .sum::<u32>()
        };
        assert_eq!(pixels(0.5), 9);
        assert_eq!(pixels(1.0), 14);
        assert_eq!(pixels(1.5), 17);
        assert_eq!(pixels(2.0), 21);
        assert_eq!(pixels(5.0), 21 + 21 + 14);
    }

    #[test]
    fn held_food_flashes_the_drumsticks_it_fills() {
        // Bread (5, 6) at 13 and full health: drumsticks 6 (the half one) to 8, each over a
        // faint container.
        let mut s = stats(13, 5.0);
        s.health = 20.0;
        let drawn = hud(&s, Some(&food(5, 6.0)), GUI, 0.65);
        assert_eq!(
            sprites(&drawn.over),
            ["hud/food_empty", "hud/food_full"].repeat(3)
        );
        // A full bar eats nothing, unless the food is always edible.
        let full = hud(&stats(20, 5.0), Some(&food(5, 6.0)), GUI, 0.65);
        assert!(sprites(&full.over).is_empty());
    }

    #[test]
    fn food_reaching_18_flashes_the_hearts_natural_regeneration_heals() {
        // 16 + 2 = 18, no saturation: one slow heal (6 exhaustion) brings the food to 17.
        assert_eq!(regen_health(18, 0.0, 0.0), 1.0);
        let drawn = hud(&stats(16, 0.0), Some(&food(2, 0.0)), GUI, 0.65);
        assert_eq!(
            sprites(&drawn.over)[..2],
            ["hud/heart/container", "hud/heart/half"]
        );
        // A golden apple's Regeneration II for 5 s heals 4, whatever the food.
        let apple = input::FoodValue {
            regeneration: Some((1, 100)),
            ..food(4, 9.6)
        };
        assert_eq!(health_gained(&stats(10, 0.0), &apple), 4.0);
        // Poison, wither or regeneration on: AppleSkin shows nothing.
        let mut poisoned = stats(10, 0.0);
        poisoned.health_effect = true;
        assert_eq!(health_gained(&poisoned, &apple), 0.0);
    }

    #[test]
    fn exhaustion_is_a_checkerboard_under_the_bar() {
        let mut s = stats(20, 5.0);
        s.exhaustion = Some(2.0);
        // The right 40 of 81 columns × 9 rows, every other pixel.
        let under = hud(&s, None, GUI, 0.0).under;
        assert_eq!(under.len(), 180);
        s.exhaustion = None;
        assert!(hud(&s, None, GUI, 0.0).under.is_empty());
    }

    #[test]
    fn the_flash_bounces_between_nothing_and_its_most() {
        let mut flash = Flash {
            unclamped: 0.0,
            up: true,
        };
        let alphas: Vec<f32> = (0..32)
            .map(|_| {
                flash.tick();
                flash.alpha()
            })
            .collect();
        assert_eq!(alphas[7], MAX_FLASH);
        assert_eq!(alphas[23], 0.0);
        // A round trip is 32 ticks.
        assert_eq!(flash.unclamped, 0.0);
    }

    #[test]
    fn nothing_without_the_hunger_bar() {
        let mut s = stats(10, 5.0);
        s.shown = false;
        let drawn = hud(&s, Some(&food(5, 6.0)), GUI, 0.65);
        assert!(drawn.over.is_empty() && drawn.under.is_empty());
    }
}
