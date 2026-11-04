// The official launcher's night sky ("Night" in docs/design-system.md, launcher/src/sky.rs) on
// Android: banded sky, stars that twinkle in steps, three layers of clouds drifting a texel at a
// time, a sun cut in slices and two skylines. The art and colours are the desktop launcher's; here
// each sprite row is drawn as runs of plain quads instead of baked images.
use horizon_client::ui_theme::Ui;
use horizon_theme::night::{NIGHT, Night, over};
use std::sync::LazyLock;

/// Dp per texel: the sky's bands, the stars and the clouds; the sun and skylines.
const TEXEL: f32 = 4.;
const LAND: f32 = 6.;
/// Each stop of the sky's share of the screen's height, top to bottom.
const SHARES: [f32; 6] = [0.2, 0.18, 0.17, 0.15, 0.15, 0.15];

struct CloudLayer {
    period: usize,
    top: f32,
    speed: f32,
    clouds: &'static [(usize, usize, &'static [&'static str])],
}

const PUFF: &[&str] = &[
    "..............############..................",
    "......##########################............",
    "..##########################################",
    "============================================",
    ".....----------------------------------.....",
];
const LONG: &[&str] = &[
    "......................##############............................",
    "..............##################################................",
    "....############################################################",
    "==============================================================..",
    "..........------------------------------------------............",
    "..............................#################.................",
    "..............................=================.................",
];
const WISP: &[&str] = &[
    "......##################......",
    "==============================",
];
const BANK: &[&str] = &[
    "............##########........................................",
    "......##########################........##############........",
    "..##################################..########################",
    "====================================..========================",
    "......-------------------------.........---------------.......",
];

const CLOUDS: [CloudLayer; 3] = [
    CloudLayer {
        period: 300,
        top: 0.36,
        speed: 0.5,
        clouds: &[(20, 0, WISP), (140, 3, PUFF), (230, 1, WISP)],
    },
    CloudLayer {
        period: 380,
        top: 0.52,
        speed: 1.,
        clouds: &[(30, 1, BANK), (200, 0, PUFF), (300, 4, WISP)],
    },
    CloudLayer {
        period: 460,
        top: 0.64,
        speed: 2.,
        clouds: &[(60, 0, LONG), (290, 2, PUFF)],
    },
];

struct Skyline {
    runs: &'static [(usize, usize)],
    props: &'static [(usize, &'static [&'static str])],
    rows: usize,
}

const TREE: &[&str] = &[".###.", "#####", "#####", ".###.", "..#..", "..#.."];
const PINE: &[&str] = &["..#..", ".###.", "..#..", ".###.", "#####", "..#.."];
const POST: &[&str] = &[".#.", "###", ".#.", ".#.", ".#."];
const TOWER: &[&str] = &["#.#", "###", "###", "###", "###", "###", "###"];

const FAR: Skyline = Skyline {
    runs: &[
        (18, 8),
        (10, 10),
        (6, 13),
        (14, 11),
        (22, 9),
        (8, 12),
        (12, 14),
        (16, 11),
        (20, 8),
        (10, 10),
        (14, 12),
        (24, 9),
        (12, 11),
        (18, 8),
    ],
    props: &[(32, TOWER), (118, PINE), (176, TOWER)],
    rows: 20,
};

const NEAR: Skyline = Skyline {
    runs: &[
        (12, 7),
        (6, 9),
        (10, 8),
        (4, 11),
        (14, 10),
        (8, 8),
        (6, 6),
        (16, 7),
        (5, 10),
        (9, 12),
        (7, 9),
        (12, 7),
        (10, 8),
        (6, 11),
        (11, 9),
        (9, 7),
        (15, 7),
    ],
    props: &[
        (5, TREE),
        (36, POST),
        (60, PINE),
        (95, TREE),
        (124, POST),
        (139, TREE),
    ],
    rows: 18,
};

const SUN_RADIUS: usize = 15;
const SUN_RISE: usize = 8;
const SUN_X: f32 = 0.72;

/// The art's colours (the desktop launcher's night appearance).
const SUN: [u32; 3] = [0xffe28c, 0xffbf66, 0xff944f];
const FAR_LAND: u32 = 0x3a1f5e;
const NEAR_LAND: u32 = 0x150f2e;
const CLOUD: [u32; 3] = [0x3b2869, 0x6e3a7e, 0xc96a86];
const LIT: [u32; 3] = [0x5a3f8a, 0x9c5a92, 0xf7a98a];
const SHADE: u32 = 0x2a1a4c;
const STAR: u32 = 0xf4f2ff;

const STAR_LEVELS: [f32; 4] = [0.95, 0.6, 0.3, 0.6];
/// Twice the desktop launcher's pace, and the clouds three times: on a phone held at arm's
/// length its texel-a-second drift reads as a still picture.
const STAR_PERIODS: [f32; 4] = [2., 3., 4., 5.];
const CLOUD_PACE: f32 = 3.;

/// `0xRRGGBB` at `alpha` (0..1) in the UI's colour format.
pub fn rgb(c: u32, alpha: f32) -> u32 {
    horizon_client::ui_theme::color::hex(c, (alpha.clamp(0., 1.) * 255.).round() as u8)
}

/// A plain quad in dp.
fn quad(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, c: u32) {
    if w > 0. && h > 0. {
        ui.rounded(x, y, w, h, 0., c);
    }
}

/// `rows` of art at `(x, y)` dp, `size` dp a texel, each run of one mark as one quad.
fn stamp(
    ui: &mut Ui,
    (x, y): (f32, f32),
    size: f32,
    rows: &[&str],
    paint: impl Fn(u8) -> Option<u32>,
) {
    for (dy, row) in rows.iter().enumerate() {
        let bytes = row.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let mark = bytes[i];
            let start = i;
            while i < bytes.len() && bytes[i] == mark {
                i += 1;
            }
            if let Some(c) = paint(mark) {
                let x0 = x + start as f32 * size;
                quad(
                    ui,
                    x0,
                    y + dy as f32 * size,
                    (i - start) as f32 * size,
                    size,
                    rgb(c, 1.),
                );
            }
        }
    }
}

/// The sky's bands, `(top, height, colour)` in dp on whole texels.
fn bands(n: &Night, h: f32) -> Vec<(f32, f32, u32)> {
    const STEPS: usize = 4;
    let rows = (h / TEXEL).ceil();
    let mut out = Vec::with_capacity(SHARES.len() * STEPS);
    let mut at = 0.;
    for (i, share) in SHARES.iter().enumerate() {
        let span = (share * rows).round().max(STEPS as f32);
        let Some(&next) = n.sky.get(i + 1) else {
            out.push((at * TEXEL, (h / TEXEL - at).max(span) * TEXEL, n.sky[i]));
            break;
        };
        let mut from = at;
        for k in 0..STEPS {
            let to = (at + span * (k + 1) as f32 / STEPS as f32).round();
            out.push((
                from * TEXEL,
                (to - from) * TEXEL,
                over(next, k as f32 / STEPS as f32, n.sky[i]),
            ));
            from = to;
        }
        at += span;
    }
    out
}

struct Star {
    x: f32,
    y: f32,
    group: usize,
    big: bool,
}

/// The same fixed star field as the desktop launcher, thinned toward the horizon.
fn stars() -> &'static [Star] {
    static STARS: LazyLock<Vec<Star>> = LazyLock::new(|| {
        let mut rng = 0x2545_f491u32;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            (rng >> 8) as f32 / (1 << 24) as f32
        };
        (0..240)
            .filter_map(|i| {
                let (x, y) = ((next() * 960.).floor(), next());
                let keep = next() > y * y;
                let big = next() < 0.08;
                keep.then_some(Star {
                    x,
                    y,
                    group: i % STAR_PERIODS.len(),
                    big,
                })
            })
            .collect()
    });
    &STARS
}

/// Which of a star group's levels shows at `t` seconds.
fn level(t: f32, period: f32, group: usize) -> f32 {
    let phase = group as f32 * 0.25;
    STAR_LEVELS
        [((t / period + phase).fract() * STAR_LEVELS.len() as f32) as usize % STAR_LEVELS.len()]
}

/// The night sky over `w` by `h` dp at `t` seconds of the launcher's clock.
pub fn draw(ui: &mut Ui, w: f32, h: f32, t: f32) {
    let n = &NIGHT;
    quad(ui, 0., 0., w, h, rgb(n.page, 1.));
    for (top, height, c) in bands(n, h) {
        quad(ui, 0., top, w, height, rgb(c, 1.));
    }
    for star in stars() {
        let (x, y) = (star.x * TEXEL, star.y * h * 0.5);
        if x >= w {
            continue;
        }
        let size = if star.big { TEXEL / 2. } else { TEXEL / 4. }.max(1.);
        let a = level(t, STAR_PERIODS[star.group], star.group);
        quad(ui, x, y, size, size, rgb(STAR, a));
    }
    for (depth, layer) in CLOUDS.iter().enumerate() {
        let strip = layer.period as f32 * TEXEL;
        let shift = (t * layer.speed * CLOUD_PACE).floor() * TEXEL % strip;
        let top = (h * layer.top / TEXEL).round() * TEXEL;
        let mut x = -shift;
        while x < w {
            for &(cx, cy, sprite) in layer.clouds {
                stamp(
                    ui,
                    (x + cx as f32 * TEXEL, top + cy as f32 * TEXEL),
                    TEXEL,
                    sprite,
                    |m| match m {
                        b'#' => Some(CLOUD[depth]),
                        b'=' => Some(LIT[depth]),
                        b'-' => Some(SHADE),
                        _ => None,
                    },
                );
            }
            x += strip;
        }
    }
    sun(ui, w * SUN_X, h - SUN_RISE as f32 * LAND);
    for (line, color, lift) in [(&FAR, FAR_LAND, 2.), (&NEAR, NEAR_LAND, 0.)] {
        skyline(ui, line, color, w, h, lift);
    }
}

/// The upper half of the sun standing on `base`, its lower half sliced by rows of sky.
fn sun(ui: &mut Ui, cx: f32, base: f32) {
    let r = SUN_RADIUS as f32;
    for y in 0..SUN_RADIUS {
        let dy = r - y as f32 - 0.5;
        let half = (r * r - dy * dy).sqrt().round();
        let depth = y as f32 / r;
        if depth > 0.5 && (SUN_RADIUS - y).is_multiple_of(3) {
            continue;
        }
        let c = SUN[if depth < 0.4 {
            0
        } else if depth < 0.7 {
            1
        } else {
            2
        }];
        let top = base - (SUN_RADIUS - y) as f32 * LAND;
        quad(
            ui,
            cx - half * LAND,
            top,
            2. * half * LAND,
            LAND,
            rgb(c, 1.),
        );
    }
}

/// A skyline along the bottom, `lift` texels above the foot, repeating across the width.
fn skyline(ui: &mut Ui, line: &Skyline, color: u32, w: f32, h: f32, lift: f32) {
    let width: usize = line.runs.iter().map(|r| r.0).sum();
    let period = width as f32 * LAND;
    let top = h - (line.rows as f32 + lift) * LAND;
    let mut tops = vec![0; width];
    let mut at = 0;
    for &(run, height) in line.runs {
        for t in &mut tops[at..at + run] {
            *t = line.rows - height;
        }
        at += run;
    }
    let fill = rgb(color, 1.);
    // The far line starts half a period in, so the two never repeat in step.
    let mut x = if lift > 0. { -period / 2. } else { 0. };
    while x < w {
        let mut at = 0;
        for &(run, height) in line.runs {
            let rows = height as f32 * LAND;
            quad(
                ui,
                x + at as f32 * LAND,
                top + line.rows as f32 * LAND - rows,
                run as f32 * LAND,
                rows,
                fill,
            );
            at += run;
        }
        for &(px, sprite) in line.props {
            let sw = sprite[0].len();
            let ground = tops[px..px + sw].iter().copied().min().unwrap_or(line.rows);
            let y = ground.saturating_sub(sprite.len());
            stamp(
                ui,
                (x + px as f32 * LAND, top + y as f32 * LAND),
                LAND,
                sprite,
                |m| (m == b'#').then_some(color),
            );
        }
        x += period;
    }
    if lift > 0. {
        quad(ui, 0., h - lift * LAND, w, lift * LAND, fill);
    }
}
