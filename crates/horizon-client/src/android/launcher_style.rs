//! The launcher's look: the official Horizon launcher's "Night" (docs/design-system.md,
//! horizon_theme::night): a pixel night sky that turns to a sunset at the bottom
//! (launcher_sky.rs), plates with pixel-cut corners over it only where text needs them, one violet
//! for the main action, a navigation rail (a bottom bar in portrait).
use crate::catalog::Page;
use horizon_client::ui_theme::{Face, Ui};

/// The launcher's colour roles, from the Night tokens. The names the pages used before stay, so
/// dialogs keep working.
pub mod color {
    use horizon_client::ui_theme::color::hex;
    /// Behind everything (`page`).
    pub const BG: u32 = hex(0x0e0b26, 255);
    /// The rail and the bottom bar (`side`).
    pub const LOW: u32 = hex(0x0a0820, 238);
    /// Plates over the sky (`card` at 84%).
    pub const CARD: u32 = hex(0x15112e, 214);
    /// A plate's 1px inner stroke (`stroke` at 12%).
    pub const RIM: u32 = hex(0xb9acff, 31);
    /// Buttons, fields and raised plates (`card_hi` at 90%).
    pub const RAISED: u32 = hex(0x231c4a, 230);
    /// Pressed plates, a switch's track when off.
    pub const HOVER: u32 = hex(0x2e2760, 240);
    /// Rules between rows.
    pub const BORDER: u32 = hex(0x2a2452, 255);
    /// Outlines and an off switch's thumb.
    pub const OUTLINE: u32 = hex(0x8a84ad, 255);
    /// Text (`ink`).
    pub const TEXT: u32 = hex(0xf4f2ff, 255);
    /// Details and captions (`muted`).
    pub const MUTED: u32 = hex(0xa59fc6, 255);
    /// The accent as text, switches and focus (`accent_hi`).
    pub const ACCENT: u32 = hex(0xa08cff, 255);
    pub const ON_ACCENT: u32 = hex(0x170c4a, 255);
    /// The avatar and the logo's tile.
    pub const ACCENT_CONTAINER: u32 = hex(0x3a2d8c, 255);
    pub const ON_ACCENT_CONTAINER: u32 = hex(0xffffff, 255);
    /// The selected navigation item's plate.
    pub const SELECTED: u32 = hex(0x2b2360, 245);
    pub const GOOD: u32 = hex(0x1f5134, 255);
    pub const ON_GOOD: u32 = hex(0xa6f2bd, 255);
    pub const WARN: u32 = hex(0x5b4300, 255);
    pub const ON_WARN: u32 = hex(0xffdf9e, 255);
    pub const BAD: u32 = hex(0x8c1d18, 255);
    pub const ON_BAD: u32 = hex(0xffdad6, 255);
    /// The one main action (Launch, Install...): `accent` with `on_accent` ink.
    pub const ACTION: u32 = hex(0x7656fa, 255);
    pub const ON_ACTION: u32 = hex(0xffffff, 255);
}

/// A plate's corner step (`PIXEL_CORNER`): plates cut each corner in two, the main action in one.
const STEP: f32 = 4.0;

/// A rectangle with its corners cut in `steps` pixel steps of [`STEP`] dp, filled with `fill`, and
/// its 1dp `stroke` following the steps (none when its alpha is 0).
pub fn pixel_plate(ui: &mut Ui, [x, y, w, h]: [f32; 4], steps: usize, fill: u32, stroke: u32) {
    let s = STEP;
    let n = steps as f32;
    // Rows from the top: row k (k < steps) is inset (steps - k) steps.
    for k in 0..steps {
        let inset = (n - k as f32) * s;
        let row = k as f32 * s;
        ui.rounded(x + inset, y + row, w - 2.0 * inset, s, 0.0, fill);
        ui.rounded(x + inset, y + h - row - s, w - 2.0 * inset, s, 0.0, fill);
    }
    ui.rounded(x, y + n * s, w, h - 2.0 * n * s, 0.0, fill);
    if stroke >> 24 == 0 {
        return;
    }
    // The stroke: the straight edges, then each step's riser and tread.
    let line = |ui: &mut Ui, x: f32, y: f32, w: f32, h: f32| ui.rounded(x, y, w, h, 0.0, stroke);
    line(ui, x + n * s, y, w - 2.0 * n * s, 1.0);
    line(ui, x + n * s, y + h - 1.0, w - 2.0 * n * s, 1.0);
    line(ui, x, y + n * s, 1.0, h - 2.0 * n * s);
    line(ui, x + w - 1.0, y + n * s, 1.0, h - 2.0 * n * s);
    for k in 0..steps {
        let (outer, inner) = ((n - k as f32 - 1.0) * s, (n - k as f32) * s);
        let row = k as f32 * s;
        for (left, top) in [(true, true), (false, true), (true, false), (false, false)] {
            let vx = if left { x + outer } else { x + w - outer - 1.0 };
            let vy = if top { y + row } else { y + h - row - s };
            line(ui, vx, vy, 1.0, s);
            let hx = if left { x + outer } else { x + w - inner };
            let hy = if top { y + row } else { y + h - row - 1.0 };
            line(ui, hx, hy, s, 1.0);
        }
    }
}

/// `c` with its alpha set to `a`.
const fn alpha(c: u32, a: u8) -> u32 {
    (c & 0x00ff_ffff) | (a as u32) << 24
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

/// The main action's plate: the accent with one pixel step cut at each corner.
pub fn action_pill(ui: &mut Ui, rect: [f32; 4], _r: f32) {
    pixel_plate(ui, rect, 1, color::ACTION, 0);
}

/// The night sky behind every page, its clouds drifting and stars twinkling on the ambient clock.
pub fn backdrop(ui: &mut Ui, w: f32, h: f32) {
    crate::launcher_sky::draw(ui, w, h, ambient());
}

/// The Play page's hero band: like the desktop launcher's home, the headline stands on the sky
/// itself, with no plate. Returns the width left for text.
pub fn hero_band(_ui: &mut Ui, [_, _, w, _]: [f32; 4]) -> f32 {
    w - 48.0
}

/// The navigation rail's width (landscape) and the bottom bar's height (portrait).
pub const RAIL: f32 = 92.0;
pub const BAR: f32 = 76.0;

/// A rounded card.
pub fn card(ui: &mut Ui, rect: [f32; 4], fill: u32) {
    pixel_plate(ui, rect, 2, fill, color::RIM);
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
    pixel_plate(ui, [x, y, s, s], 2, color::RAISED, color::RIM);
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
    pixel_plate(
        ui,
        [cx - 30.0, y - 2.0, 60.0, 36.0],
        1,
        color::SELECTED,
        color::RIM,
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
