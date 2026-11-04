//! The launcher's look ("Nightly"): glass cards on a deep indigo night with violet, blue and pink
//! glows, a gradient hero with a synthwave sun, a navigation rail (a bottom bar in portrait),
//! pill buttons and rounded cards.
use crate::catalog::Page;
use horizon_client::ui_theme::{Face, Ui};

/// The launcher's colour roles. The names the pages used before stay, so dialogs keep working.
pub mod color {
    use horizon_client::ui_theme::color::hex;
    /// The page behind everything: the night sky's base.
    pub const BG: u32 = hex(0x0b0c1d, 255);
    /// The sky at the bottom of the screen.
    pub const BG_LOW: u32 = hex(0x150e2e, 255);
    /// The rail and the bottom bar: glass over the sky.
    pub const LOW: u32 = hex(0x0e1024, 214);
    /// Cards: glass, nearly opaque so text stays crisp.
    pub const CARD: u32 = hex(0x151931, 236);
    /// A card's 1px rim of light.
    pub const RIM: u32 = hex(0xffffff, 22);
    /// Tonal buttons and fields.
    pub const RAISED: u32 = hex(0x232849, 255);
    /// Pressed plates, a switch's track when off.
    pub const HOVER: u32 = hex(0x2e3460, 255);
    /// Dividers.
    pub const BORDER: u32 = hex(0x2c3156, 255);
    /// Outlines and an off switch's thumb.
    pub const OUTLINE: u32 = hex(0x7a7fa8, 255);
    pub const TEXT: u32 = hex(0xeef0ff, 255);
    /// Secondary text.
    pub const MUTED: u32 = hex(0xa6abd1, 255);
    /// The one filled action per screen.
    pub const ACCENT: u32 = hex(0x9d8cff, 255);
    pub const ON_ACCENT: u32 = hex(0x170c4a, 255);
    /// The avatar and the logo tile.
    pub const ACCENT_CONTAINER: u32 = hex(0x3b2d91, 255);
    pub const ON_ACCENT_CONTAINER: u32 = hex(0xf1edff, 255);
    /// The selected rail item's indicator.
    pub const SELECTED: u32 = hex(0x2f2a6e, 255);
    pub const GOOD: u32 = hex(0x1f5134, 255);
    pub const ON_GOOD: u32 = hex(0xa6f2bd, 255);
    pub const WARN: u32 = hex(0x5b4300, 255);
    pub const ON_WARN: u32 = hex(0xffdf9e, 255);
    pub const BAD: u32 = hex(0x8c1d18, 255);
    pub const ON_BAD: u32 = hex(0xffdad6, 255);
    /// The filled action (Launch, Install...): a violet to orchid pill with white ink.
    pub const ACTION_LEFT: u32 = hex(0x7357ff, 255);
    pub const ACTION_RIGHT: u32 = hex(0xc046d6, 255);
    pub const ON_ACTION: u32 = hex(0xffffff, 255);
    /// The hero's dusk, left to right.
    pub const DUSK_LEFT: u32 = hex(0x3a24b8, 255);
    pub const DUSK_RIGHT: u32 = hex(0xb0379f, 255);
    /// The sun, top to bottom.
    pub const SUN_TOP: u32 = hex(0xffd56b, 255);
    pub const SUN_BOTTOM: u32 = hex(0xff4f8b, 255);
    /// The glows in the sky (their centres; they fade out to nothing).
    pub const GLOW_VIOLET: u32 = hex(0x6b4dff, 92);
    pub const GLOW_BLUE: u32 = hex(0x2f6bff, 64);
    pub const GLOW_PINK: u32 = hex(0xff3fa0, 44);
}

/// `c` with its alpha set to `a`.
const fn alpha(c: u32, a: u8) -> u32 {
    (c & 0x00ff_ffff) | (a as u32) << 24
}

/// The colour `t` of the way from `a` to `b` (every channel, alpha too).
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    (0..4).fold(0, |out, i| {
        let (ca, cb) = ((a >> (i * 8)) & 0xff, (b >> (i * 8)) & 0xff);
        let c = (ca as f32 + (cb as f32 - ca as f32) * t).round() as u32;
        out | c << (i * 8)
    })
}

/// A quad in dp with its own colour at each corner (`corners` and `colors` in the same order,
/// round the shape), blended across it by the GPU.
fn shade(ui: &mut Ui, corners: [[f32; 2]; 4], colors: [u32; 4]) {
    let s = ui.batch.scale;
    let (ox, oy, d) = (ui.origin.0, ui.origin.1, ui.density);
    let gui = corners.map(|[x, y]| [((x + ox) * d).round() / s, ((y + oy) * d).round() / s]);
    let start = ui.batch.vertices.len();
    ui.batch.fill_quad(ui.atlas, gui, u32::MAX);
    // fill_quad's two triangles: corners 0 1 2 and 0 2 3.
    for (v, i) in ui.batch.vertices[start..]
        .iter_mut()
        .zip([0, 1, 2, 0, 2, 3])
    {
        v.color = colors[i];
    }
}

/// A rectangle whose corners have their own colours: top left, top right, bottom right, bottom
/// left.
pub fn gradient(ui: &mut Ui, [x, y, w, h]: [f32; 4], colors: [u32; 4]) {
    shade(ui, [[x, y], [x + w, y], [x + w, y + h], [x, y + h]], colors);
}

/// A rounded rectangle shading from `left` to `right` (opaque colours: its caps overlap).
pub fn rounded_across(ui: &mut Ui, [x, y, w, h]: [f32; 4], r: f32, left: u32, right: u32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    ui.rounded(x, y, 2.0 * r, h, r, left);
    ui.rounded(x + w - 2.0 * r, y, 2.0 * r, h, r, right);
    gradient(ui, [x + r, y, w - 2.0 * r, h], [left, right, right, left]);
}

/// A soft round glow: `color` at the centre fading to nothing at radius `r`.
pub fn glow(ui: &mut Ui, cx: f32, cy: f32, r: f32, color: u32) {
    const SEGMENTS: usize = 32;
    let rim = alpha(color, 0);
    // Two rings: the inner half keeps most of the light, so the falloff looks round, not conical.
    let mid = alpha(color, ((color >> 24) as f32 * 0.45) as u8);
    let point = |k: usize, radius: f32| {
        let a = k as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        [cx + radius * a.cos(), cy + radius * a.sin()]
    };
    for k in 0..SEGMENTS {
        let (a0, a1) = (point(k, r * 0.5), point(k + 1, r * 0.5));
        shade(ui, [[cx, cy], a0, a1, a1], [color, mid, mid, mid]);
        let (b0, b1) = (point(k, r), point(k + 1, r));
        shade(ui, [a0, b0, b1, a1], [mid, rim, rim, mid]);
    }
}

/// The launcher's animations: a clock for the slow ambient motion (glows drifting, the Launch
/// button breathing), the page switch in progress and the last touch's ripple. The launcher draws
/// at 60 frames a second only while [`Motion::busy`], and at 10 otherwise.
pub struct Motion {
    clock: std::time::Instant,
    page: Option<(Page, std::time::Instant)>,
    ripple: Option<([f32; 2], std::time::Instant)>,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            clock: std::time::Instant::now(),
            page: None,
            ripple: None,
        }
    }
}
impl Motion {
    /// How long a page switch and a ripple take, in seconds.
    const PAGE: f32 = 0.3;
    const RIPPLE: f32 = 0.45;
    /// The page shown before `from` was left for another.
    pub fn page_changed(&mut self, from: Page) {
        self.page = Some((from, std::time::Instant::now()));
    }
    /// A touch at `at` (dp).
    pub fn touched(&mut self, at: [f32; 2]) {
        self.ripple = Some((at, std::time::Instant::now()));
    }
    /// Seconds since the launcher started: the ambient motion's clock.
    pub fn time(&self) -> f32 {
        self.clock.elapsed().as_secs_f32()
    }
    /// The page being left and how far the switch is, eased (0 just left, 1 done).
    pub fn page_progress(&self) -> Option<(Page, f32)> {
        let (from, at) = self.page?;
        let t = at.elapsed().as_secs_f32() / Self::PAGE;
        (t < 1.0).then(|| (from, ease_out(t)))
    }
    /// Whether something moves fast enough to need every frame.
    pub fn busy(&self) -> bool {
        self.page_progress().is_some()
            || self
                .ripple
                .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < Self::RIPPLE)
    }
    /// The ripple of the last touch, over everything: a glow that grows and fades.
    pub fn draw_ripple(&self, ui: &mut Ui) {
        let Some(([x, y], at)) = self.ripple else {
            return;
        };
        let t = at.elapsed().as_secs_f32() / Self::RIPPLE;
        if t >= 1.0 {
            return;
        }
        let e = ease_out(t);
        let a = ((1.0 - t) * 70.0) as u8;
        glow(ui, x, y, 14.0 + 46.0 * e, alpha(color::ACCENT, a));
    }
    /// The page's entrance: a veil of the sky fading off it as it settles.
    pub fn draw_page_veil(&self, ui: &mut Ui, rect: [f32; 4]) {
        if let Some((_, p)) = self.page_progress() {
            ui.rounded(
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                0.0,
                alpha(color::BG, ((1.0 - p) * 230.0) as u8),
            );
        }
    }
    /// How far the page's content still sits to the right as it slides in (dp).
    pub fn page_slide(&self) -> f32 {
        self.page_progress().map_or(0.0, |(_, p)| (1.0 - p) * 28.0)
    }
}

/// Cubic ease-out: fast start, gentle stop.
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// The ambient clock of this frame (seconds), set by the launcher before it draws: the pieces
/// that breathe or drift read it, so their callers need not pass it along.
static AMBIENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub fn set_ambient(seconds: f32) {
    AMBIENT.store(seconds.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
fn ambient() -> f32 {
    f32::from_bits(AMBIENT.load(std::sync::atomic::Ordering::Relaxed))
}

/// The filled action's pill: a soft glow, breathing slowly, under a violet to orchid gradient.
pub fn action_pill(ui: &mut Ui, [x, y, w, h]: [f32; 4], r: f32) {
    let breath = (ambient() * 2.2).sin() * 0.5 + 0.5;
    glow(
        ui,
        x + w / 2.0,
        y + h * 0.75,
        (w * 0.6).max(h) * (0.92 + 0.12 * breath),
        alpha(color::ACTION_RIGHT, (44.0 + 34.0 * breath) as u8),
    );
    rounded_across(ui, [x, y, w, h], r, color::ACTION_LEFT, color::ACTION_RIGHT);
}

/// The night sky behind every page: a dark gradient with violet, blue and pink glows that drift.
pub fn backdrop(ui: &mut Ui, w: f32, h: f32) {
    gradient(
        ui,
        [0.0, 0.0, w, h],
        [color::BG, color::BG, color::BG_LOW, color::BG_LOW],
    );
    let big = w.max(h);
    let t = ambient();
    let drift = |speed: f32, phase: f32| (t * speed + phase).sin();
    glow(
        ui,
        w * (0.78 + 0.05 * drift(0.11, 0.0)),
        h * (-0.05 + 0.04 * drift(0.07, 1.3)),
        big * 0.62,
        color::GLOW_VIOLET,
    );
    glow(
        ui,
        w * (0.05 + 0.05 * drift(0.09, 2.1)),
        h * (1.02 + 0.03 * drift(0.13, 0.4)),
        big * 0.5,
        color::GLOW_BLUE,
    );
    glow(
        ui,
        w * (0.98 + 0.03 * drift(0.08, 4.0)),
        h * (0.92 + 0.05 * drift(0.1, 2.7)),
        big * (0.34 + 0.03 * drift(0.17, 0.9)),
        color::GLOW_PINK,
    );
}

/// A synthwave sun: the top half of a disc of radius `r` standing on `base`, shading from
/// yellow to pink, cut by bands that widen towards the horizon.
pub fn sun(ui: &mut Ui, cx: f32, base: f32, r: f32) {
    const SLICES: usize = 24;
    let step = r / SLICES as f32;
    for i in 0..SLICES {
        let (t0, t1) = (i as f32 / SLICES as f32, (i + 1) as f32 / SLICES as f32);
        // The lower half of the sun is cut: a gap that grows towards the horizon.
        let gap = if t0 >= 0.45 {
            step * (t0 - 0.35) * 1.6
        } else {
            0.0
        };
        let (y0, y1) = (
            base - r + i as f32 * step,
            base - r + (i + 1) as f32 * step - gap,
        );
        if y1 <= y0 {
            continue;
        }
        let half = |y: f32| (r * r - (base - y).powi(2)).max(0.0).sqrt();
        let (top, bottom) = (
            mix(color::SUN_TOP, color::SUN_BOTTOM, t0),
            mix(color::SUN_TOP, color::SUN_BOTTOM, t1),
        );
        let (h0, h1) = (half(y0), half(y1));
        shade(
            ui,
            [[cx - h0, y0], [cx + h0, y0], [cx + h1, y1], [cx - h1, y1]],
            [top, top, bottom, bottom],
        );
    }
    glow(
        ui,
        cx,
        base - r * 0.3,
        r * 1.3,
        alpha(color::SUN_BOTTOM, 34),
    );
}

/// The Play page's hero band: dusk from violet to magenta, square at the bottom where the card's
/// body joins it, with the sun on the right when there is room. Returns the width left for text.
pub fn hero_band(ui: &mut Ui, [x, y, w, h]: [f32; 4]) -> f32 {
    rounded_across(ui, [x, y, w, h], 16.0, color::DUSK_LEFT, color::DUSK_RIGHT);
    gradient(
        ui,
        [x, y + h - 16.0, w, 16.0],
        [
            color::DUSK_LEFT,
            color::DUSK_RIGHT,
            color::DUSK_RIGHT,
            color::DUSK_LEFT,
        ],
    );
    // The horizon: a thin bright line where the sun sets.
    let r = (h * 0.62).min(w * 0.16);
    if w >= 480.0 && r >= 36.0 {
        let cx = x + w - r - 28.0;
        sun(ui, cx, y + h - 6.0, r);
        gradient(
            ui,
            [x + w * 0.45, y + h - 6.0, w * 0.55, 2.0],
            [
                alpha(color::SUN_BOTTOM, 0),
                alpha(color::SUN_TOP, 200),
                alpha(color::SUN_TOP, 200),
                alpha(color::SUN_BOTTOM, 0),
            ],
        );
        w - 2.0 * r - 76.0
    } else {
        w - 48.0
    }
}

/// The navigation rail's width (landscape) and the bottom bar's height (portrait).
pub const RAIL: f32 = 92.0;
pub const BAR: f32 = 76.0;

/// A rounded card.
pub fn card(ui: &mut Ui, [x, y, w, h]: [f32; 4], fill: u32) {
    ui.rounded(x, y, w, h, 16.0, fill);
    ui.border(x, y, w, h, 16.0, color::RIM);
}

/// A small filled label ("Built in", "Needs build").
pub fn badge(ui: &mut Ui, x: f32, y: f32, label: &str, fill: u32, ink: u32) -> f32 {
    let w = ui.measure(Face::Medium, 13.0, label) + 20.0;
    ui.rounded(x, y, w, 26.0, 13.0, fill);
    ui.text(Face::Medium, 13.0, x + 10.0, y + 4.0, ink, label);
    w
}

/// A disc of diameter `d` at `(x, y)`.
fn disc(ui: &mut Ui, x: f32, y: f32, d: f32, fill: u32) {
    ui.rounded(x, y, d, d, d / 2.0, fill);
}

/// The initial in a round avatar of diameter `d`.
pub fn avatar(ui: &mut Ui, x: f32, y: f32, d: f32, name: &str) {
    disc(ui, x, y, d, color::ACCENT_CONTAINER);
    let initial: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .collect();
    let size = d * 0.45;
    let w = ui.measure(Face::SemiBold, size, &initial);
    ui.text(
        Face::SemiBold,
        size,
        x + (d - w) / 2.0,
        y + (d - size * 1.45) / 2.0,
        color::ON_ACCENT_CONTAINER,
        &initial,
    );
}

/// Horizon's tile: a rounded square with the "H".
pub fn logo(ui: &mut Ui, x: f32, y: f32, s: f32) {
    glow(
        ui,
        x + s / 2.0,
        y + s / 2.0,
        s * 1.1,
        alpha(color::DUSK_LEFT, 120),
    );
    rounded_across(
        ui,
        [x, y, s, s],
        s * 0.3,
        color::DUSK_LEFT,
        color::DUSK_RIGHT,
    );
    let size = s * 0.5;
    let w = ui.measure(Face::SemiBold, size, "H");
    ui.text(
        Face::SemiBold,
        size,
        x + (s - w) / 2.0,
        y + (s - size * 1.45) / 2.0,
        color::ON_ACCENT_CONTAINER,
        "H",
    );
}

/// The page's icon in a `s`-sized box, drawn from rounded rectangles (the atlas has no icon font).
pub fn icon(ui: &mut Ui, page: Page, x: f32, y: f32, s: f32, ink: u32) {
    let u = s / 24.0;
    match page {
        Page::Play => {
            // A triangle pointing right, as narrowing columns.
            for i in 0..8 {
                let t = i as f32 / 8.0;
                let h = 16.0 * u * (1.0 - t);
                ui.rounded(
                    x + (7.0 + i as f32 * 1.6) * u,
                    y + 12.0 * u - h / 2.0,
                    1.7 * u,
                    h,
                    0.0,
                    ink,
                );
            }
        }
        Page::Mods => {
            // Four tiles, one lifted: the "extension" glyph.
            for (cx, cy) in [(4.0, 4.0), (13.0, 4.0), (4.0, 13.0)] {
                ui.rounded(x + cx * u, y + cy * u, 7.0 * u, 7.0 * u, 1.5 * u, ink);
            }
            ui.rounded(x + 14.0 * u, y + 14.0 * u, 6.0 * u, 6.0 * u, 3.0 * u, ink);
        }
        Page::Packs => {
            // Stacked layers.
            for (i, inset) in [6.0, 4.0, 2.0].into_iter().enumerate() {
                ui.rounded(
                    x + inset * u,
                    y + (5.0 + i as f32 * 5.5) * u,
                    (24.0 - 2.0 * inset) * u,
                    3.5 * u,
                    1.5 * u,
                    ink,
                );
            }
        }
        Page::Shaders => {
            // A sun: a disc and its rays.
            disc(ui, x + 8.0 * u, y + 8.0 * u, 8.0 * u, ink);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                disc(
                    ui,
                    x + (12.0 + 9.0 * a.cos() - 1.5) * u,
                    y + (12.0 + 9.0 * a.sin() - 1.5) * u,
                    3.0 * u,
                    ink,
                );
            }
        }
        Page::Installed => {
            // A folder.
            ui.rounded(x + 3.0 * u, y + 5.0 * u, 8.0 * u, 4.0 * u, 1.5 * u, ink);
            ui.rounded(x + 3.0 * u, y + 7.5 * u, 18.0 * u, 12.0 * u, 2.0 * u, ink);
        }
        Page::Settings => {
            // Sliders ("tune").
            for (row, knob) in [(6.0, 14.0), (12.0, 6.0), (18.0, 11.0)] {
                ui.rounded(
                    x + 3.0 * u,
                    y + (row - 1.0) * u,
                    18.0 * u,
                    2.0 * u,
                    1.0 * u,
                    ink,
                );
                disc(ui, x + (knob - 2.5) * u, y + (row - 2.5) * u, 5.0 * u, ink);
            }
        }
    }
}

/// A page's short name under its icon.
pub fn nav_label(page: Page) -> &'static str {
    match page {
        Page::Installed => "Files",
        Page::Settings => "Setup",
        _ => page.name(),
    }
}

/// The selected item's indicator pill, centred on `cx` with its top at `y`.
fn nav_indicator(ui: &mut Ui, cx: f32, y: f32) {
    glow(ui, cx, y + 16.0, 44.0, alpha(color::DUSK_LEFT, 90));
    rounded_across(
        ui,
        [cx - 28.0, y, 56.0, 32.0],
        16.0,
        color::ACCENT_CONTAINER,
        color::SELECTED,
    );
}

/// One navigation item: the icon and the label, centred on `cx`.
fn nav_item(ui: &mut Ui, page: Page, cx: f32, y: f32, selected: bool) {
    let ink = if selected { color::TEXT } else { color::MUTED };
    icon(ui, page, cx - 12.0, y + 4.0, 24.0, ink);
    let label = nav_label(page);
    let w = ui.measure(Face::Medium, 12.0, label);
    ui.text(Face::Medium, 12.0, cx - w / 2.0, y + 36.0, ink, label);
}

/// Where the indicator sits along the navigation: at `current`'s place, or on its way there
/// from the page left (`from`, with the switch's eased progress).
fn indicator_at(current: Page, from: Option<(Page, f32)>, place: impl Fn(usize) -> f32) -> f32 {
    let index = |p: Page| Page::ALL.iter().position(|q| *q == p).unwrap_or(0);
    let to = place(index(current));
    match from {
        Some((page, p)) => place(index(page)) + (to - place(index(page))) * p,
        None => to,
    }
}

/// The navigation rail on the left (wide screens): the logo, then one item per page, the
/// indicator sliding to the page chosen. Returns each page's touch rectangle.
pub fn rail(
    ui: &mut Ui,
    current: Page,
    from: Option<(Page, f32)>,
    h: f32,
) -> Vec<([f32; 4], Page)> {
    ui.rounded(0.0, 0.0, RAIL, h, 0.0, color::LOW);
    ui.rounded(RAIL - 1.0, 0.0, 1.0, h, 0.0, color::RIM);
    logo(ui, (RAIL - 48.0) / 2.0, 20.0, 48.0);
    let step = ((h - 104.0) / Page::ALL.len() as f32).clamp(56.0, 76.0);
    let place = |i: usize| 100.0 + i as f32 * step;
    nav_indicator(ui, RAIL / 2.0, indicator_at(current, from, place));
    Page::ALL
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let y = place(i);
            nav_item(ui, *page, RAIL / 2.0, y, *page == current);
            ([4.0, y - 4.0, RAIL - 8.0, 60.0], *page)
        })
        .collect()
}

/// The bottom navigation bar (portrait), the indicator sliding to the page chosen. Returns each
/// page's touch rectangle.
pub fn bar(
    ui: &mut Ui,
    current: Page,
    from: Option<(Page, f32)>,
    w: f32,
    h: f32,
) -> Vec<([f32; 4], Page)> {
    let top = h - BAR;
    ui.rounded(0.0, top, w, BAR, 0.0, color::LOW);
    ui.rounded(0.0, top, w, 1.0, 0.0, color::RIM);
    let step = w / Page::ALL.len() as f32;
    let place = |i: usize| i as f32 * step + step / 2.0;
    nav_indicator(ui, indicator_at(current, from, place), top + 10.0);
    Page::ALL
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let x = i as f32 * step;
            nav_item(ui, *page, x + step / 2.0, top + 10.0, *page == current);
            ([x, top, step, BAR], *page)
        })
        .collect()
}

/// The account chip of the top bar: avatar, name and sign-in state.
pub fn account_chip(ui: &mut Ui, [x, y, w, h]: [f32; 4], status: &str, signed_in: bool) {
    ui.rounded(x, y, w, h, h / 2.0, color::CARD);
    ui.border(x, y, w, h, h / 2.0, color::RIM);
    let (state, name) = split_status(status);
    avatar(ui, x + 6.0, y + (h - 36.0) / 2.0, 36.0, name);
    let name = ui.fit_text(Face::Medium, 15.0, name, w - 64.0);
    ui.text(Face::Medium, 15.0, x + 50.0, y + 4.0, color::TEXT, &name);
    let state = ui.fit_text(Face::Regular, 12.0, state, w - 64.0);
    ui.text(
        Face::Regular,
        12.0,
        x + 50.0,
        y + 24.0,
        if signed_in {
            color::ACCENT
        } else {
            color::MUTED
        },
        &state,
    );
}

/// "Signed in: Alex" as ("Signed in", "Alex"); without a name, a sign-in prompt.
pub fn split_status(status: &str) -> (&str, &str) {
    match status.split_once(": ") {
        Some((state, name)) if !name.trim().is_empty() => (state, name),
        _ if status.starts_with("Signing") => (status, "Account"),
        _ => ("Tap to sign in", "Not signed in"),
    }
}

/// A native mod's status as a badge's fill and ink.
pub fn native_badge(status: &horizon_client::content_files::NativeStatus) -> (u32, u32) {
    use horizon_client::content_files::NativeStatus as S;
    match status {
        S::BuiltIn => (color::GOOD, color::ON_GOOD),
        S::NeedsBuild | S::OtherVersion(_) => (color::WARN, color::ON_WARN),
        S::Bundled => (color::RAISED, color::MUTED),
        S::Incompatible(_) => (color::BAD, color::ON_BAD),
    }
}
