//! The launcher's look: Material 3 tonal surfaces on a dark scheme built from Horizon's violet,
//! a navigation rail (a bottom bar in portrait), pill buttons and rounded cards.
use crate::catalog::Page;
use horizon_client::ui_theme::{Face, Ui};

/// The launcher's colour roles. The names the pages used before stay, so dialogs keep working.
pub mod color {
    use horizon_client::ui_theme::color::hex;
    /// The page behind everything (M3 `surface`).
    pub const BG: u32 = hex(0x131318, 255);
    /// The rail and the bottom bar (`surfaceContainerLow`).
    pub const LOW: u32 = hex(0x1b1b21, 255);
    /// Cards (`surfaceContainer`).
    pub const CARD: u32 = hex(0x1f1f26, 255);
    /// Tonal buttons and fields (`surfaceContainerHigh`).
    pub const RAISED: u32 = hex(0x2a2931, 255);
    /// Pressed plates, a switch's track when off (`surfaceContainerHighest`).
    pub const HOVER: u32 = hex(0x35343c, 255);
    /// Dividers (`outlineVariant`).
    pub const BORDER: u32 = hex(0x47464f, 255);
    /// Outlines and an off switch's thumb (`outline`).
    pub const OUTLINE: u32 = hex(0x928f9a, 255);
    pub const TEXT: u32 = hex(0xe5e1ea, 255);
    /// Secondary text (`onSurfaceVariant`).
    pub const MUTED: u32 = hex(0xc9c5d1, 255);
    /// The one filled action per screen (`primary`).
    pub const ACCENT: u32 = hex(0xc8bfff, 255);
    pub const ON_ACCENT: u32 = hex(0x2e2276, 255);
    /// The avatar and the logo tile (`primaryContainer`).
    pub const ACCENT_CONTAINER: u32 = hex(0x463b8f, 255);
    pub const ON_ACCENT_CONTAINER: u32 = hex(0xe5deff, 255);
    /// The selected rail item's indicator (`secondaryContainer`).
    pub const SELECTED: u32 = hex(0x48455c, 255);
    pub const GOOD: u32 = hex(0x1f5134, 255);
    pub const ON_GOOD: u32 = hex(0xa6f2bd, 255);
    pub const WARN: u32 = hex(0x5b4300, 255);
    pub const ON_WARN: u32 = hex(0xffdf9e, 255);
    pub const BAD: u32 = hex(0x8c1d18, 255);
    pub const ON_BAD: u32 = hex(0xffdad6, 255);
}

/// The navigation rail's width (landscape) and the bottom bar's height (portrait).
pub const RAIL: f32 = 92.0;
pub const BAR: f32 = 76.0;

/// A rounded card.
pub fn card(ui: &mut Ui, [x, y, w, h]: [f32; 4], fill: u32) {
    ui.rounded(x, y, w, h, 16.0, fill);
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
    ui.rounded(x, y, s, s, s * 0.3, color::ACCENT_CONTAINER);
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

/// One navigation item: the indicator pill, the icon and the label, centred on `cx`.
fn nav_item(ui: &mut Ui, page: Page, cx: f32, y: f32, selected: bool) {
    if selected {
        ui.rounded(cx - 28.0, y, 56.0, 32.0, 16.0, color::SELECTED);
    }
    let ink = if selected { color::TEXT } else { color::MUTED };
    icon(ui, page, cx - 12.0, y + 4.0, 24.0, ink);
    let label = nav_label(page);
    let w = ui.measure(Face::Medium, 12.0, label);
    ui.text(Face::Medium, 12.0, cx - w / 2.0, y + 36.0, ink, label);
}

/// The navigation rail on the left (wide screens): the logo, then one item per page.
/// Returns each page's touch rectangle.
pub fn rail(ui: &mut Ui, current: Page, h: f32) -> Vec<([f32; 4], Page)> {
    ui.rounded(0.0, 0.0, RAIL, h, 0.0, color::LOW);
    logo(ui, (RAIL - 48.0) / 2.0, 20.0, 48.0);
    let step = ((h - 104.0) / Page::ALL.len() as f32).clamp(56.0, 76.0);
    Page::ALL
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let y = 100.0 + i as f32 * step;
            nav_item(ui, *page, RAIL / 2.0, y, *page == current);
            ([4.0, y - 4.0, RAIL - 8.0, 60.0], *page)
        })
        .collect()
}

/// The bottom navigation bar (portrait). Returns each page's touch rectangle.
pub fn bar(ui: &mut Ui, current: Page, w: f32, h: f32) -> Vec<([f32; 4], Page)> {
    let top = h - BAR;
    ui.rounded(0.0, top, w, BAR, 0.0, color::LOW);
    let step = w / Page::ALL.len() as f32;
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
    ui.rounded(x, y, w, h, h / 2.0, color::RAISED);
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
