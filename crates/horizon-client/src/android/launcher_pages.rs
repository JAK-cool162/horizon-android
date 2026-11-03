use super::*;
use crate::launcher_style::{self as style, color};
use horizon_client::osk::Output;
use winit::{
    event::{KeyEvent, Touch},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    Page(Page),
    Play,
    Search,
    ShaderSource(crate::shader_catalog::Origin),
    ModSource(crate::shader_catalog::Origin),
    Field(&'static str),
    FinishNameEdit,
    Install(usize),
    Import(horizon_client::content_files::Kind),
    OpenFolder(horizon_client::content_files::Kind),
    ToggleSection(usize),
    ToggleLocal(usize),
    DeleteLocal(usize),
    ConfirmDeleteLocal,
    ConfirmExit,
    Next,
    Previous,
    Profile,
    Logs,
    RecordPerformance,
    ExportPerformance,
    ExportDiagnostics,
    Account,
    SignIn,
    DeviceSignIn,
    CopyLoginCode,
    CancelLogin,
    ReviewApp,
    Repository,
    SignOut,
    LoginLink,
    CloseLogin,
    Renderer,
    RenderScale,
    ChooseRenderScale(crate::display::RenderScale),
    Orientation,
    InputHost,
    ChooseInputHost(crate::catalog::InputHost),
    ChooseOrientation(crate::catalog::Orientation),
    ChooseRenderer(crate::catalog::RendererMode),
    ChooseProfile(u32),
    Dismiss,
    Immersive,
    Cutout,
    Aspect,
    ChooseAspect(crate::display::Aspect),
    DisplayHz,
    ChooseDisplayHz(u16),
    CacheBudget,
    ChooseCacheBudget(u32),
    ToggleBundled(&'static str),
    WorldFiles,
    GameFiles,
    WorkerLimit,
    ChooseWorkerLimit(u32),
    PerformanceHints,
    GpuProfile,
    BuildNative,
    ConfirmBuildNative,
    InstallNative,
    OpenNativeRun,
    OpenTokenPage,
    PasteToken,
}

pub(super) fn button(
    ui: &mut Ui,
    targets: &mut Vec<([f32; 4], Action)>,
    rect: [f32; 4],
    label: &str,
    action: Action,
    accent: bool,
) {
    let [x, y, w, h] = rect;
    // Fields and long value rows read from the left; actions are centred pills (Material 3).
    let field = matches!(
        action,
        Action::Field(_)
            | Action::ToggleSection(_)
            | Action::Renderer
            | Action::RenderScale
            | Action::WorkerLimit
            | Action::CacheBudget
            | Action::GameFiles
            | Action::WorldFiles
            | Action::Profile
            | Action::Orientation
            | Action::InputHost
            | Action::Aspect
            | Action::DisplayHz
            | Action::GpuProfile
            | Action::RecordPerformance
            | Action::ExportDiagnostics
            | Action::ExportPerformance
    );
    let radius = if field { 12.0 } else { (h / 2.0).min(22.0) };
    ui.rounded(
        x,
        y,
        w,
        h,
        radius,
        if accent { color::ACCENT } else { color::RAISED },
    );
    let padding = if field {
        12.0
    } else {
        (w * 0.12).clamp(4.0, 16.0)
    };
    let font = if w < 80.0 { 13.0 } else { 16.0 };
    let label = ui.fit_text(Face::Medium, font, label, w - 2.0 * padding);
    let left = if field {
        x + padding
    } else {
        x + (w - ui.measure(Face::Medium, font, &label)) / 2.0
    };
    ui.text(
        Face::Medium,
        font,
        left,
        y + ((h - font * 1.45) / 2.0).max(0.0),
        if accent {
            color::ON_ACCENT
        } else {
            color::TEXT
        },
        &label,
    );
    targets.push((rect, action));
}
/// A Material 3 switch with its state written beside it.
fn toggle(
    ui: &mut Ui,
    targets: &mut Vec<([f32; 4], Action)>,
    rect: [f32; 4],
    on: bool,
    action: Action,
) {
    let [x, y, w, h] = rect;
    ui.text(
        Face::Regular,
        14.0,
        x,
        y + 11.0,
        color::MUTED,
        if on { "On" } else { "Off" },
    );
    let (tx, ty) = (x + w - 56.0, y + ((h - 32.0) / 2.0).max(0.0));
    if on {
        ui.rounded(tx, ty, 52.0, 32.0, 16.0, color::ACCENT);
        ui.rounded(tx + 24.0, ty + 4.0, 24.0, 24.0, 12.0, color::ON_ACCENT);
    } else {
        ui.rounded(tx, ty, 52.0, 32.0, 16.0, color::HOVER);
        ui.border(tx, ty, 52.0, 32.0, 16.0, color::OUTLINE);
        ui.rounded(tx + 8.0, ty + 8.0, 16.0, 16.0, 8.0, color::OUTLINE);
    }
    targets.push(([x, y, w, h], action));
}
impl Launcher {
    /// The same spatial focus algorithm as the game's controller menus.
    pub(super) fn navigate(&mut self, key: &KeyEvent) -> bool {
        self.navigate_key(&key.logical_key)
    }
    pub(super) fn navigate_key(&mut self, key: &winit::keyboard::Key) -> bool {
        use horizon_client::gamepad::{self, Button};
        use winit::keyboard::{Key, NamedKey};
        if self.pending_focus.is_some() {
            return true;
        }
        if self.catalog.editing.is_some() || self.targets.is_empty() {
            return false;
        }
        let button = match key {
            Key::Named(NamedKey::ArrowUp) => Button::DPadUp,
            Key::Named(NamedKey::ArrowDown) => Button::DPadDown,
            Key::Named(NamedKey::ArrowLeft) => Button::DPadLeft,
            Key::Named(NamedKey::ArrowRight) => Button::DPadRight,
            Key::Named(NamedKey::Tab) => {
                self.focus = Some(self.focus.map_or(0, |i| (i + 1) % self.targets.len()));
                return true;
            }
            _ => return false,
        };
        if matches!(button, Button::DPadUp | Button::DPadDown)
            && let Some((_, action)) = self.focus.and_then(|i| self.targets.get(i))
            && let Some(index) = self.list_targets.iter().position(|(_, a)| a == action)
        {
            let rects: Vec<_> = self.list_targets.iter().map(|(r, _)| *r).collect();
            if let Some(next) = gamepad::focus_step_strict(rects[index], button, &rects)
                && let Some(view) = self.list_view
            {
                let (rect, action) = self.list_targets[next];
                let scroll = if let Some(chooser) = &mut self.chooser {
                    &mut chooser.scroll
                } else {
                    &mut self.catalog.scroll
                };
                *scroll = super::gesture::reveal(*scroll, rect, view);
                self.pending_focus = Some(action);
                self.focus = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                return true;
            }
        }
        self.focus = Some(
            if let Some(index) = self.focus.filter(|i| *i < self.targets.len()) {
                let rects: Vec<_> = self.targets.iter().map(|(rect, _)| *rect).collect();
                gamepad::focus_step_rect(rects[index], button, &rects).unwrap_or(index)
            } else {
                0
            },
        );
        true
    }

    pub(super) fn draw_pages(&mut self) -> Result<()> {
        let (Some(r), Some(gui)) = (&mut self.renderer, &self.gui) else {
            return Ok(());
        };
        if r.suspended {
            return Ok(());
        }
        self.targets.clear();
        self.list_targets.clear();
        self.list_view = None;
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let mut ui = Ui::mobile(&gui.atlas, &mut b, (0.0, 0.0));
        // OEM navigation bars can remain visible even in fullscreen (keypad phones).
        // Inset the launcher layout only; keep surface and input coordinates intact.
        if let Some(window) = &self.window {
            use winit::platform::android::WindowExtAndroid;
            let rect = window.content_rect();
            let input = winit::platform::android::native_input_scale();
            if rect.bottom > 0 {
                ui.h = ui.h.min(rect.bottom as f32 * input[1] as f32 / ui.density);
            }
            if rect.right > 0 {
                ui.w = ui.w.min(rect.right as f32 * input[0] as f32 / ui.density);
            }
        }
        if let Some(bottom) = crate::native_ime::viewport_bottom() {
            ui.h = ui.h.min(bottom as f32 / ui.density);
        }
        let (sw, sh) = (ui.w, ui.h);
        let scale = ui.density / ui.batch.scale;
        ui.batch
            .fill(&gui.atlas, 0.0, 0.0, sw * scale, sh * scale, color::BG);
        // Navigation: a rail on wide screens, a bottom bar in portrait.
        let wide = sw >= 700.0;
        let nav = if wide {
            style::rail(&mut ui, self.catalog.page, sh)
        } else {
            style::bar(&mut ui, self.catalog.page, sw, sh)
        };
        self.targets.extend(
            nav.into_iter()
                .map(|(rect, page)| (rect, Action::Page(page))),
        );
        let left = if wide { style::RAIL } else { 0.0 };
        // The top bar: the title, and the account on every page but Play (its panel has it).
        let title_x = left + if wide { 32.0 } else { 20.0 };
        let title_size = if wide { 26.0 } else { 22.0 };
        ui.text(
            Face::SemiBold,
            title_size,
            title_x,
            18.0,
            color::TEXT,
            "Horizon",
        );
        let version_x = title_x + ui.measure(Face::SemiBold, title_size, "Horizon") + 10.0;
        let chip_w = if wide { 280.0 } else { 196.0 };
        let version = ui.fit_text(
            Face::Regular,
            13.0,
            &format!("Android {}", crate::backend::VERSION),
            (sw - chip_w - 32.0 - version_x).max(0.0),
        );
        ui.text(Face::Regular, 13.0, version_x, 30.0, color::MUTED, &version);
        if self.catalog.page != Page::Play {
            let rect = [sw - chip_w - 16.0, 14.0, chip_w, 48.0];
            style::account_chip(
                &mut ui,
                rect,
                &self.login.status(&self.catalog.config.name),
                self.login.signed_in,
            );
            self.targets.push((rect, Action::Account));
        }
        // The pages lay out in their own panel: right of the rail, above the bar, and
        // `SHIFT` higher than under the old tab row the top bar replaces.
        const SHIFT: f32 = 48.0;
        ui.origin = (left, -SHIFT);
        ui.w = sw - left;
        ui.h = sh + SHIFT - if wide { 0.0 } else { style::BAR };
        let first_target = self.targets.len();
        let (w, h) = (ui.w, ui.h);
        let compact = w < 700.0;
        let page = self.catalog.page;
        if page == Page::Play {
            use horizon_client::official_install::InstallStage;
            let installed = horizon_client::official_install::ready(&self.home);
            let installing = self.worker.is_some();
            let progress = *self.install.lock().unwrap();
            let current = InstallStage::ALL
                .iter()
                .position(|s| *s == progress.stage)
                .unwrap_or(0);
            let fraction = if progress.total > 0 {
                progress.done as f32 / progress.total as f32
            } else {
                0.0
            };
            let overall = (current as f32 + fraction.clamp(0.0, 1.0)) / 6.0;
            let (top, foot, gap, card_h, launch_h) = (134.0, h - 64.0, 16.0, 84.0, 72.0);
            // The launch panel (account, version, mods, Launch) beside the overview; stacked
            // under it in portrait.
            let (panel, hero) = if compact {
                let panel_h = 2.0 * (card_h + gap) + launch_h;
                (
                    [24.0, foot - panel_h, w - 48.0, panel_h],
                    [24.0, top, w - 48.0, foot - panel_h - gap - top],
                )
            } else {
                let pw = (w * 0.34).clamp(280.0, 380.0);
                (
                    [w - 32.0 - pw, top, pw, foot - top],
                    [32.0, top, w - 64.0 - pw - gap, foot - top],
                )
            };
            let [hx, hy, hw, hh] = hero;
            style::card(&mut ui, hero, color::CARD);
            let band = (hh * 0.4).clamp(110.0, 150.0);
            ui.rounded(hx, hy, hw, band, 16.0, color::ACCENT_CONTAINER);
            ui.rounded(hx, hy + band - 16.0, hw, 16.0, 0.0, color::ACCENT_CONTAINER);
            let headline = if installing {
                "Getting your game ready"
            } else if installed {
                "Ready to play"
            } else if hw < 420.0 {
                "Your Java worlds"
            } else {
                "Your Java worlds, on Android"
            };
            let headline = ui.fit_text(Face::SemiBold, 28.0, headline, hw - 48.0);
            ui.text(
                Face::SemiBold,
                28.0,
                hx + 24.0,
                hy + band - 92.0,
                color::ON_ACCENT_CONTAINER,
                &headline,
            );
            ui.text(
                Face::Regular,
                15.0,
                hx + 24.0,
                hy + band - 44.0,
                color::ON_ACCENT_CONTAINER,
                &format!("Minecraft 26.3  ·  Profile {}", self.catalog.config.profile),
            );
            let body = hy + band + 20.0;
            if installing {
                let rows = hh - band - 80.0;
                if rows >= 6.0 * 26.0 {
                    let step = (rows / 6.0).min(34.0);
                    for (index, stage) in InstallStage::ALL.iter().enumerate() {
                        let y = body + index as f32 * step;
                        let (fill, ink) = if index < current {
                            (color::GOOD, color::ON_GOOD)
                        } else if index == current {
                            (color::ACCENT, color::ON_ACCENT)
                        } else {
                            (color::RAISED, color::MUTED)
                        };
                        ui.rounded(hx + 24.0, y + 1.0, 22.0, 22.0, 11.0, fill);
                        let n = format!("{}", index + 1);
                        let nw = ui.measure(Face::Medium, 12.0, &n);
                        ui.text(Face::Medium, 12.0, hx + 35.0 - nw / 2.0, y + 3.0, ink, &n);
                        ui.text(
                            Face::Regular,
                            16.0,
                            hx + 58.0,
                            y,
                            if index == current {
                                color::TEXT
                            } else {
                                color::MUTED
                            },
                            stage.label(),
                        );
                    }
                } else {
                    ui.text(
                        Face::Regular,
                        18.0,
                        hx + 24.0,
                        body,
                        color::TEXT,
                        progress.stage.label(),
                    );
                    ui.text(
                        Face::Regular,
                        15.0,
                        hx + 24.0,
                        body + 32.0,
                        color::MUTED,
                        &format!("Step {} of 6", current + 1),
                    );
                }
                let detail = match progress.stage {
                    InstallStage::Download => format!(
                        "{:.1} / {:.1} MiB  - verified downloads",
                        progress.done as f64 / 1048576.0,
                        progress.total as f64 / 1048576.0
                    ),
                    InstallStage::Controls => format!(
                        "{} / {} control files verified",
                        progress.done, progress.total
                    ),
                    _ => "Please keep Horizon open. Downloaded files are kept for retry.".into(),
                };
                let detail = ui.fit_text(Face::Regular, 14.0, &detail, hw - 48.0);
                ui.text(
                    Face::Regular,
                    14.0,
                    hx + 24.0,
                    hy + hh - 40.0,
                    color::MUTED,
                    &detail,
                );
            } else {
                let rows = if installed {
                    [
                        "Open a world or join a Java server",
                        "Touch setup: Options > Controls",
                        "Find packs, .hmod and .hnmod in the tabs",
                    ]
                } else {
                    [
                        "Download resources from Mojang",
                        "Install and verify on this phone",
                        "Play offline or sign in for servers",
                    ]
                };
                for (index, line) in rows.iter().enumerate() {
                    let y = body + index as f32 * 40.0;
                    if y + 30.0 > hy + hh {
                        break;
                    }
                    ui.rounded(hx + 24.0, y, 26.0, 26.0, 13.0, color::RAISED);
                    let n = format!("{}", index + 1);
                    let nw = ui.measure(Face::Medium, 13.0, &n);
                    ui.text(
                        Face::Medium,
                        13.0,
                        hx + 37.0 - nw / 2.0,
                        y + 4.0,
                        color::TEXT,
                        &n,
                    );
                    let line = ui.fit_text(Face::Regular, 16.0, line, hw - 90.0);
                    ui.text(Face::Regular, 16.0, hx + 62.0, y + 2.0, color::MUTED, &line);
                }
            }
            // The panel.
            let [px, py, pw, ph] = panel;
            let account = [px, py, pw, card_h];
            style::card(&mut ui, account, color::CARD);
            let status = self.login.status(&self.catalog.config.name);
            let (state, name) = style::split_status(&status);
            style::avatar(&mut ui, px + 16.0, py + 18.0, 48.0, name);
            let name = ui.fit_text(Face::Medium, 18.0, name, pw - 96.0);
            ui.text(Face::Medium, 18.0, px + 78.0, py + 16.0, color::TEXT, &name);
            let state = ui.fit_text(Face::Regular, 13.0, state, pw - 96.0);
            ui.text(
                Face::Regular,
                13.0,
                px + 78.0,
                py + 46.0,
                if self.login.signed_in {
                    color::ACCENT
                } else {
                    color::MUTED
                },
                &state,
            );
            self.targets.push((account, Action::Account));
            let version = [px, py + card_h + gap, pw, card_h];
            style::card(&mut ui, version, color::CARD);
            ui.text(
                Face::Medium,
                18.0,
                px + 16.0,
                version[1] + 14.0,
                color::TEXT,
                "Minecraft 26.3",
            );
            let about = ui.fit_text(
                Face::Regular,
                13.0,
                &format!(
                    "Horizon Android {}  ·  Profile {}",
                    crate::backend::VERSION,
                    self.catalog.config.profile
                ),
                pw - 32.0,
            );
            ui.text(
                Face::Regular,
                13.0,
                px + 16.0,
                version[1] + 46.0,
                color::MUTED,
                &about,
            );
            self.targets.push((version, Action::Profile));
            if ph >= 3.0 * (card_h + gap) + launch_h {
                let mods = [px, py + 2.0 * (card_h + gap), pw, card_h];
                style::card(&mut ui, mods, color::CARD);
                let fixed =
                    self.catalog.bundled_enabled.len() + crate::catalog::built_native_mods().len();
                let sandboxed = self
                    .catalog
                    .local
                    .iter()
                    .filter(|e| e.kind == horizon_client::content_files::Kind::Mods && e.enabled)
                    .count();
                ui.text(
                    Face::Medium,
                    18.0,
                    px + 16.0,
                    mods[1] + 14.0,
                    color::TEXT,
                    "Mods",
                );
                let summary = ui.fit_text(
                    Face::Regular,
                    13.0,
                    &format!("{fixed} native  ·  {sandboxed} .hmod enabled"),
                    pw - 32.0,
                );
                ui.text(
                    Face::Regular,
                    13.0,
                    px + 16.0,
                    mods[1] + 46.0,
                    color::MUTED,
                    &summary,
                );
                self.targets.push((mods, Action::Page(Page::Installed)));
            }
            let launch = [px, py + ph - launch_h, pw, launch_h];
            if installing {
                style::card(&mut ui, launch, color::CARD);
                let label = ui.fit_text(Face::Medium, 16.0, progress.stage.label(), pw - 32.0);
                ui.text(
                    Face::Medium,
                    16.0,
                    px + 16.0,
                    launch[1] + 12.0,
                    color::TEXT,
                    &label,
                );
                ui.rounded(
                    px + 16.0,
                    launch[1] + 48.0,
                    pw - 32.0,
                    6.0,
                    3.0,
                    color::HOVER,
                );
                if overall > 0.0 {
                    ui.rounded(
                        px + 16.0,
                        launch[1] + 48.0,
                        (pw - 32.0) * overall,
                        6.0,
                        3.0,
                        color::ACCENT,
                    );
                }
            } else {
                ui.rounded(px, launch[1], pw, launch_h, 20.0, color::ACCENT);
                let label = if installed {
                    "Launch"
                } else if self.error.is_some() {
                    "Retry installation"
                } else {
                    "Download Minecraft 26.3"
                };
                let size = if installed { 24.0 } else { 18.0 };
                let label = ui.fit_text(Face::SemiBold, size, label, pw - 32.0);
                let lw = ui.measure(Face::SemiBold, size, &label);
                ui.text(
                    Face::SemiBold,
                    size,
                    px + (pw - lw) / 2.0,
                    launch[1] + (launch_h - size * 1.45) / 2.0,
                    color::ON_ACCENT,
                    &label,
                );
                self.targets.push((launch, Action::Play));
            }
        } else if page.is_catalog() {
            ui.text(
                Face::Medium,
                18.0,
                32.0,
                142.0,
                color::TEXT,
                if page == Page::Mods {
                    "Horizon Mods"
                } else if page == Page::Shaders {
                    if w < 360.0 { "Shaders" } else { "Shader Packs" }
                } else {
                    "Resource Packs"
                },
            );
            button(
                &mut ui,
                &mut self.targets,
                [32.0, 174.0, w - 240.0, 44.0],
                &format!(
                    "{}{}",
                    if page == Page::Mods {
                        "Search / mod URL: "
                    } else {
                        "Search: "
                    },
                    self.catalog.query
                ),
                Action::Field("search"),
                false,
            );
            button(
                &mut ui,
                &mut self.targets,
                [w - 190.0, 174.0, 158.0, 44.0],
                if page == Page::Mods && self.catalog.query.trim().starts_with("https://") {
                    "Import mod"
                } else {
                    "Search"
                },
                Action::Search,
                true,
            );
            if matches!(page, Page::Shaders | Page::Mods) {
                use crate::shader_catalog::Origin;
                let width = ((w - 185.0) / 2.0).clamp(58.0, 120.0);
                for (i, origin) in [Origin::Horizon, Origin::Modrinth].into_iter().enumerate() {
                    let rect = [w - 32.0 - (2 - i) as f32 * width, 134.0, width - 5.0, 32.0];
                    button(
                        &mut ui,
                        &mut self.targets,
                        rect,
                        origin.source().label(),
                        if page == Page::Mods {
                            Action::ModSource(origin)
                        } else {
                            Action::ShaderSource(origin)
                        },
                        false,
                    );
                    if (if page == Page::Mods {
                        self.catalog.config.mod_source
                    } else {
                        self.catalog.config.shader_source
                    }) == origin
                    {
                        ui.border(rect[0], rect[1], rect[2], rect[3], 16.0, color::ACCENT);
                    }
                }
            }
            let hint = match page {
                Page::Mods
                    if self.catalog.config.mod_source
                        == crate::shader_catalog::Origin::Modrinth =>
                {
                    "Modrinth Java mods are browse-only; Fabric/Forge/NeoForge cannot run here"
                }
                Page::Mods => {
                    "Search Horizon mods, paste an HTTPS .hmod/.hnmod link or import a file"
                }
                Page::Shaders
                    if self.catalog.config.shader_source
                        == crate::shader_catalog::Origin::Modrinth =>
                {
                    "Modrinth shader ZIPs / experimental compatibility; some packs may not work"
                }
                Page::Shaders => "Horizon native .hshader packs",
                _ => "Verified downloads / enable in Installed",
            };
            let hint = ui.fit_text(Face::Regular, 14.0, hint, w - 64.0);
            ui.text(Face::Regular, 14.0, 32.0, 270.0, color::MUTED, &hint);
            let kind = match page {
                Page::Mods => horizon_client::content_files::Kind::Mods,
                Page::Shaders => horizon_client::content_files::Kind::Shaders,
                _ => horizon_client::content_files::Kind::Packs,
            };
            let file_button_width = ((w - 72.0) / 2.0).min(158.0);
            button(
                &mut ui,
                &mut self.targets,
                [32.0, 222.0, file_button_width, 38.0],
                "Open folder",
                Action::OpenFolder(kind),
                false,
            );
            button(
                &mut ui,
                &mut self.targets,
                [w - 32.0 - file_button_width, 222.0, file_button_width, 38.0],
                "Import file",
                Action::Import(kind),
                false,
            );
            let bundled: Vec<_> = if page == Page::Mods
                && self.catalog.config.mod_source == crate::shader_catalog::Origin::Horizon
            {
                let query = self.catalog.query.to_lowercase();
                horizon_client::mod_list::ANDROID_BUNDLED
                    .into_iter()
                    .filter(|(id, title, _)| {
                        id.contains(&query) || title.to_lowercase().contains(&query)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let catalog_rows: Vec<_> = self
                .catalog
                .results
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    !(page == Page::Mods
                        && p.source == crate::browser::SourceId::Horizon
                        && horizon_client::mod_list::ANDROID_BUNDLED
                            .iter()
                            .any(|(id, _, _)| *id == p.id))
                })
                .collect();
            let row_count = bundled.len() + catalog_rows.len();
            let top = 296.0;
            let bottom = h - 112.0;
            self.catalog.scroll = self
                .catalog
                .scroll
                .clamp(0.0, (row_count as f32 * 78.0 - (bottom - top)).max(0.0));
            self.list_view = Some([
                top,
                bottom,
                (row_count as f32 * 78.0 - (bottom - top)).max(0.0),
            ]);
            for (row, (id, title, description)) in bundled.iter().enumerate() {
                let y = top + row as f32 * 78.0 - self.catalog.scroll;
                if y < top - 1.0 || y + 68.0 > bottom {
                    continue;
                }
                style::card(&mut ui, [16.0, y - 8.0, w - 32.0, 66.0], color::CARD);
                let title = ui.fit_text(Face::Medium, 19.0, title, w - 214.0);
                let summary = ui.fit_text(Face::Regular, 14.0, description, w - 214.0);
                ui.text(Face::Medium, 19.0, 32.0, y, color::TEXT, &title);
                ui.text(Face::Regular, 14.0, 32.0, y + 29.0, color::MUTED, &summary);
                button(
                    &mut ui,
                    &mut self.targets,
                    [w - 150.0, y, 118.0, 42.0],
                    if self.catalog.bundled_enabled.contains(*id) {
                        "Disable"
                    } else {
                        "Enable"
                    },
                    Action::ToggleBundled(id),
                    false,
                );
            }
            for (row, (i, p)) in catalog_rows.into_iter().enumerate() {
                let y = top + (row + bundled.len()) as f32 * 78.0 - self.catalog.scroll;
                let download = self.catalog.download_label(p);
                if download.is_none()
                    && !(p.kind == crate::browser::Kind::Mod
                        && p.source == crate::browser::SourceId::Modrinth)
                {
                    self.list_targets
                        .push(([w - 150.0, y, 118.0, 70.0], Action::Install(i)));
                }
                if y < top || y + 70.0 > bottom {
                    continue;
                }
                style::card(&mut ui, [16.0, y - 8.0, w - 32.0, 66.0], color::CARD);
                let title = ui.fit_text(Face::Medium, 19.0, &p.title, w - 214.0);
                let summary = ui.fit_text(Face::Regular, 14.0, &p.summary, w - 214.0);
                ui.text(Face::Medium, 19.0, 32.0, y, color::TEXT, &title);
                ui.text(Face::Regular, 14.0, 32.0, y + 29.0, color::MUTED, &summary);
                if crate::catalog::source_only_mod(p) {
                    let x = 40.0 + ui.measure(Face::Medium, 19.0, &title);
                    if x + 70.0 < w - 160.0 {
                        style::badge(&mut ui, x, y, ".hnmod", color::RAISED, color::MUTED);
                    }
                }
                if let Some(state) = download {
                    ui.text(Face::Medium, 16.0, w - 150.0, y + 12.0, color::TEXT, &state);
                } else if p.kind == crate::browser::Kind::Mod
                    && p.source == crate::browser::SourceId::Modrinth
                {
                    ui.text(
                        Face::Regular,
                        14.0,
                        w - 150.0,
                        y + 12.0,
                        color::MUTED,
                        "Java mod",
                    );
                } else if p.kind == crate::browser::Kind::Mod
                    && !crate::catalog::source_only_mod(p)
                    && !cfg!(target_pointer_width = "64")
                {
                    ui.text(
                        Face::Regular,
                        14.0,
                        w - 150.0,
                        y + 12.0,
                        color::MUTED,
                        "64-bit required",
                    );
                } else {
                    button(
                        &mut ui,
                        &mut self.targets,
                        [w - 150.0, y, 118.0, 42.0],
                        self.catalog.install_button(p),
                        Action::Install(i),
                        false,
                    );
                }
            }
            button(
                &mut ui,
                &mut self.targets,
                [32.0, h - 100.0, 120.0, 40.0],
                "Previous",
                Action::Previous,
                false,
            );
            button(
                &mut ui,
                &mut self.targets,
                [166.0, h - 100.0, 120.0, 40.0],
                "Next",
                Action::Next,
                false,
            );
        } else if page == Page::Installed {
            use horizon_client::content_files::Kind;
            let top = 156.0;
            let bottom = h - 100.0;
            let row_height = if compact { 110.0 } else { 72.0 };
            let total = Kind::ALL
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    48.0 + if self.catalog.sections[i] {
                        (self.catalog.local.iter().filter(|e| e.kind == *k).count()
                            + usize::from(*k == Kind::NativeMods)) as f32
                            * row_height
                    } else {
                        0.0
                    }
                })
                .sum::<f32>();
            let maximum = (total - (bottom - top)).max(0.0);
            self.catalog.scroll = self.catalog.scroll.clamp(0.0, maximum);
            self.list_view = Some([top, bottom, maximum]);
            let mut offset = 0.0;
            for (section, kind) in Kind::ALL.into_iter().enumerate() {
                let count = self.catalog.local.iter().filter(|e| e.kind == kind).count();
                let y = top + offset - self.catalog.scroll;
                let label = format!(
                    "{} {} ({count})",
                    if self.catalog.sections[section] {
                        "-"
                    } else {
                        "+"
                    },
                    kind.label()
                );
                self.list_targets
                    .push(([32.0, y, w - 210.0, 40.0], Action::ToggleSection(section)));
                self.list_targets
                    .push(([w - 165.0, y, 133.0, 40.0], Action::Import(kind)));
                if y >= top && y + 40.0 <= bottom {
                    button(
                        &mut ui,
                        &mut self.targets,
                        [32.0, y, w - 210.0, 40.0],
                        &label,
                        Action::ToggleSection(section),
                        false,
                    );
                    button(
                        &mut ui,
                        &mut self.targets,
                        [w - 165.0, y, 133.0, 40.0],
                        "Import",
                        Action::Import(kind),
                        false,
                    );
                }
                offset += 48.0;
                if !self.catalog.sections[section] {
                    continue;
                }
                if kind == Kind::NativeMods {
                    use crate::native_build::Phase;
                    let y = top + offset - self.catalog.scroll;
                    offset += row_height;
                    let phase = self.catalog.native_build.phase();
                    let running = self.catalog.native_build.running();
                    let (main, other) = match &phase {
                        _ if running => (
                            self.catalog
                                .native_build
                                .url()
                                .map(|_| ("View", Action::OpenNativeRun)),
                            None,
                        ),
                        Phase::Ready => (
                            Some(("Install", Action::InstallNative)),
                            Some(("Build again", Action::BuildNative)),
                        ),
                        _ => (Some(("Build APK", Action::BuildNative)), None),
                    };
                    let button_y = y + if compact { 55.0 } else { 0.0 };
                    if let Some((_, action)) = main {
                        self.list_targets
                            .push(([w - 165.0, button_y, 133.0, 40.0], action));
                    }
                    if let Some((_, action)) = other {
                        self.list_targets.push((
                            [
                                if compact { 32.0 } else { w - 300.0 },
                                button_y,
                                120.0,
                                40.0,
                            ],
                            action,
                        ));
                    }
                    if y >= top && y + row_height - 4.0 <= bottom {
                        style::card(
                            &mut ui,
                            [16.0, y - 6.0, w - 32.0, row_height - 8.0],
                            color::ACCENT_CONTAINER,
                        );
                        let title = ui.fit_text(
                            Face::Medium,
                            19.0,
                            "Build an APK with these mods",
                            if compact { w - 64.0 } else { w - 364.0 },
                        );
                        ui.text(
                            Face::Medium,
                            19.0,
                            32.0,
                            y,
                            color::ON_ACCENT_CONTAINER,
                            &title,
                        );
                        let detail = match &phase {
                            Phase::Idle => {
                                "On your GitHub fork: the phone cannot compile Rust".to_owned()
                            }
                            Phase::Working(text) => text.clone(),
                            Phase::Ready => {
                                "Ready: install it (it installs as Horizon Android dev)".into()
                            }
                            Phase::Failed(error) => error.clone(),
                        };
                        let detail = ui.fit_text(
                            Face::Regular,
                            14.0,
                            &detail,
                            if compact { w - 64.0 } else { w - 364.0 },
                        );
                        ui.text(
                            Face::Regular,
                            14.0,
                            32.0,
                            y + 28.0,
                            color::ON_ACCENT_CONTAINER,
                            &detail,
                        );
                        if let Some((label, action)) = main {
                            button(
                                &mut ui,
                                &mut self.targets,
                                [w - 165.0, button_y, 133.0, 40.0],
                                label,
                                action,
                                !running,
                            );
                        }
                        if let Some((label, action)) = other {
                            button(
                                &mut ui,
                                &mut self.targets,
                                [
                                    if compact { 32.0 } else { w - 300.0 },
                                    button_y,
                                    120.0,
                                    40.0,
                                ],
                                label,
                                action,
                                false,
                            );
                        }
                    }
                }
                for (i, item) in self
                    .catalog
                    .local
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.kind == kind)
                {
                    let y = top + offset - self.catalog.scroll;
                    offset += row_height;
                    self.list_targets.push((
                        [
                            if compact { 32.0 } else { w - 300.0 },
                            y,
                            120.0,
                            row_height - 4.0,
                        ],
                        Action::ToggleLocal(i),
                    ));
                    self.list_targets.push((
                        [w - 165.0, y, 133.0, row_height - 4.0],
                        Action::DeleteLocal(i),
                    ));
                    if y < top || y + row_height - 4.0 > bottom {
                        continue;
                    }
                    style::card(
                        &mut ui,
                        [16.0, y - 6.0, w - 32.0, row_height - 8.0],
                        color::CARD,
                    );
                    let title = ui.fit_text(
                        Face::Medium,
                        19.0,
                        &item.title,
                        if compact { w - 64.0 } else { w - 364.0 },
                    );
                    ui.text(Face::Medium, 19.0, 32.0, y, color::TEXT, &title);
                    let detail = match &item.native {
                        Some(status) if status.plays() => "Native mod / plays in this APK",
                        Some(horizon_client::content_files::NativeStatus::Bundled) => {
                            "Native mod / this APK's fixed copy plays instead"
                        }
                        Some(horizon_client::content_files::NativeStatus::Incompatible(_)) => {
                            "Native mod / an APK of this source cannot build it"
                        }
                        Some(_) => "Native mod / build an APK with it",
                        None => kind.label(),
                    };
                    let detail = ui.fit_text(
                        Face::Regular,
                        14.0,
                        detail,
                        if compact { w - 64.0 } else { w - 364.0 },
                    );
                    ui.text(Face::Regular, 14.0, 32.0, y + 28.0, color::MUTED, &detail);
                    if let Some(status) = &item.native {
                        let (fill, ink) = style::native_badge(status);
                        let label = ui.fit_text(Face::Medium, 13.0, &status.label(), 110.0);
                        style::badge(
                            &mut ui,
                            if compact { 32.0 } else { w - 300.0 },
                            y + if compact { 62.0 } else { 7.0 },
                            &label,
                            fill,
                            ink,
                        );
                    } else if kind == Kind::Mods && !cfg!(target_pointer_width = "64") {
                        ui.text(
                            Face::Regular,
                            14.0,
                            if compact { 32.0 } else { w - 300.0 },
                            y + if compact { 62.0 } else { 12.0 },
                            color::MUTED,
                            "64-bit required",
                        );
                    } else {
                        toggle(
                            &mut ui,
                            &mut self.targets,
                            [
                                if compact { 32.0 } else { w - 300.0 },
                                y + if compact { 55.0 } else { 0.0 },
                                120.0,
                                40.0,
                            ],
                            item.enabled,
                            Action::ToggleLocal(i),
                        );
                    }
                    button(
                        &mut ui,
                        &mut self.targets,
                        [w - 165.0, y + if compact { 55.0 } else { 0.0 }, 133.0, 40.0],
                        "Remove",
                        Action::DeleteLocal(i),
                        false,
                    );
                }
            }
        } else {
            let c = &self.catalog.config;
            ui.text(
                Face::Medium,
                24.0,
                32.0,
                156.0,
                color::TEXT,
                "Launch settings",
            );
            let row_height = if compact { 82.0 } else { 66.0 };
            self.catalog.scroll = self
                .catalog
                .scroll
                .clamp(0.0, (21.0 * row_height - (h - 130.0 - 195.0)).max(0.0));
            self.list_view = Some([
                195.0,
                h - 130.0,
                (21.0 * row_height - (h - 130.0 - 195.0)).max(0.0),
            ]);
            for (i, (title, value, action)) in [
                ("Rendering", c.renderer.label().to_owned(), Action::Renderer),
                (
                    "Client resolution",
                    if self.catalog.editing == Some("render_scale") {
                        self.catalog.render_scale_text.clone()
                    } else {
                        format!("{}%", c.render_scale.percent())
                    },
                    Action::RenderScale,
                ),
                (
                    "Chunk worker limit",
                    if c.worker_limit == 0 {
                        "No additional limit".into()
                    } else {
                        c.worker_limit.to_string()
                    },
                    Action::WorkerLimit,
                ),
                (
                    "Reusable RAM cache",
                    if c.cache_budget_mib == 0 {
                        "Engine default (80 MiB)".into()
                    } else {
                        format!("{} MiB", c.cache_budget_mib)
                    },
                    Action::CacheBudget,
                ),
                (
                    "Game files",
                    "Open in system Files".into(),
                    Action::GameFiles,
                ),
                (
                    "World files",
                    "Import / copy / back up in Files".into(),
                    Action::WorldFiles,
                ),
                ("Profile", format!("Profile {}", c.profile), Action::Profile),
                (
                    "Orientation",
                    c.orientation.label().to_owned(),
                    Action::Orientation,
                ),
                ("Input", c.input_host.label().to_owned(), Action::InputHost),
                (
                    "Immersive fullscreen",
                    if c.screen.immersive { "On" } else { "Off" }.into(),
                    Action::Immersive,
                ),
                (
                    "Avoid camera cutout",
                    if c.screen.avoid_cutout { "On" } else { "Off" }.into(),
                    Action::Cutout,
                ),
                (
                    "Aspect ratio",
                    c.screen.aspect.label().into(),
                    Action::Aspect,
                ),
                (
                    "Display refresh rate",
                    c.device.display_label(),
                    Action::DisplayHz,
                ),
                (
                    "Performance hints",
                    if c.device.performance_hints {
                        "On (ADPF)"
                    } else {
                        "Off"
                    }
                    .into(),
                    Action::PerformanceHints,
                ),
                (
                    "GPU profiling",
                    if c.device.gpu_profile {
                        "On / included in report"
                    } else {
                        "Off"
                    }
                    .into(),
                    Action::GpuProfile,
                ),
                (
                    "Performance recording",
                    if c.record_performance {
                        "On / up to 30 min"
                    } else {
                        "Off"
                    }
                    .into(),
                    Action::RecordPerformance,
                ),
                (
                    "Diagnostic report",
                    "Export one ZIP to share".into(),
                    Action::ExportDiagnostics,
                ),
                (
                    "Raw performance data",
                    "Advanced / JSONL in Reports".into(),
                    Action::ExportPerformance,
                ),
                (
                    "Server",
                    if c.server.is_empty() {
                        "Singleplayer / tap to enter server".into()
                    } else {
                        c.server.clone()
                    },
                    Action::Field("server"),
                ),
                (
                    "GitHub repository",
                    if !c.github_repo.is_empty() {
                        c.github_repo.clone()
                    } else if let Some(repo) = crate::native_build::repository("") {
                        format!("{repo} (this APK's)")
                    } else {
                        "Your fork, for native mod builds".into()
                    },
                    Action::Field("github_repo"),
                ),
                (
                    "GitHub token",
                    if self.catalog.editing == Some("github_token") {
                        "*".repeat(self.catalog.github_token_text.chars().count())
                    } else if self.catalog.github_token_saved {
                        "Saved / tap to replace (empty removes it)".into()
                    } else {
                        "Not set / fine-grained, Contents + Actions".into()
                    },
                    Action::Field("github_token"),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let y = 195.0 + i as f32 * row_height - self.catalog.scroll;
                self.list_targets.push((
                    [
                        if compact { 32.0 } else { w * 0.36 },
                        y,
                        if compact { w - 64.0 } else { w * 0.64 - 32.0 },
                        if compact { 70.0 } else { 46.0 },
                    ],
                    action,
                ));
                if y < 185.0 || y + if compact { 70.0 } else { 46.0 } > h - 130.0 {
                    continue;
                }
                ui.text(
                    Face::Regular,
                    16.0,
                    32.0,
                    y + if compact { 0.0 } else { 13.0 },
                    color::MUTED,
                    title,
                );
                if matches!(
                    action,
                    Action::PerformanceHints | Action::Immersive | Action::Cutout
                ) {
                    toggle(
                        &mut ui,
                        &mut self.targets,
                        [
                            if compact { 32.0 } else { w * 0.36 },
                            y + if compact { 24.0 } else { 0.0 },
                            if compact { w - 64.0 } else { w * 0.64 - 32.0 },
                            46.0,
                        ],
                        match action {
                            Action::PerformanceHints => c.device.performance_hints,
                            Action::Immersive => c.screen.immersive,
                            Action::Cutout => c.screen.avoid_cutout,
                            _ => c.device.gpu_profile,
                        },
                        action,
                    );
                } else {
                    button(
                        &mut ui,
                        &mut self.targets,
                        [
                            if compact { 32.0 } else { w * 0.36 },
                            y + if compact { 24.0 } else { 0.0 },
                            if compact { w - 64.0 } else { w * 0.64 - 32.0 },
                            46.0,
                        ],
                        &format!(
                            "{} {}",
                            value,
                            if matches!(
                                action,
                                Action::Renderer
                                    | Action::Aspect
                                    | Action::DisplayHz
                                    | Action::RenderScale
                                    | Action::Profile
                                    | Action::Orientation
                                    | Action::InputHost
                            ) {
                                "v"
                            } else {
                                ""
                            }
                        ),
                        action,
                        false,
                    );
                }
                ui.rounded(
                    32.0,
                    y + if compact { 78.0 } else { 62.0 },
                    w - 64.0,
                    1.0,
                    0.0,
                    color::BORDER,
                );
            }
            button(
                &mut ui,
                &mut self.targets,
                [w * 0.5 + 6.0, h - 114.0, w * 0.5 - 38.0, 42.0],
                "Microsoft / Xbox account",
                Action::Account,
                true,
            );
            button(
                &mut ui,
                &mut self.targets,
                [32.0, h - 114.0, w * 0.5 - 38.0, 42.0],
                "Diagnostics",
                Action::Logs,
                false,
            );
        }
        // The latest message, as a snackbar: on a raised plate, in the text colour.
        let status = self.error.as_deref().unwrap_or(&self.catalog.status).trim();
        if !status.is_empty() {
            let width = (w - 200.0).max(40.0);
            let lines: Vec<String> = status
                .chars()
                .collect::<Vec<_>>()
                .chunks(110)
                .take(2)
                .map(|line| {
                    ui.fit_text(
                        Face::Regular,
                        14.0,
                        &line.iter().collect::<String>(),
                        width - 32.0,
                    )
                })
                .collect();
            let text_w = lines
                .iter()
                .map(|l| ui.measure(Face::Regular, 14.0, l))
                .fold(0.0f32, f32::max);
            let plate_h = 14.0 + lines.len() as f32 * 18.0;
            ui.rounded(
                24.0,
                h - 30.0 - plate_h,
                (text_w + 32.0).min(width),
                plate_h + 8.0,
                12.0,
                if self.error.is_some() {
                    color::BAD
                } else {
                    color::RAISED
                },
            );
            for (i, line) in lines.iter().enumerate() {
                ui.text(
                    Face::Regular,
                    14.0,
                    40.0,
                    h - 30.0 - plate_h + 7.0 + i as f32 * 18.0,
                    if self.error.is_some() {
                        color::ON_BAD
                    } else {
                        color::TEXT
                    },
                    line,
                );
            }
        }
        button(
            &mut ui,
            &mut self.targets,
            [w - 142.0, h - 52.0, 110.0, 32.0],
            "GitHub",
            Action::Repository,
            false,
        );
        // Back to the whole screen: the panel's touch rectangles move with it.
        ui.origin = (0.0, 0.0);
        ui.w = sw;
        ui.h = sh;
        for (rect, _) in self.targets[first_target..]
            .iter_mut()
            .chain(self.list_targets.iter_mut())
        {
            rect[0] += left;
            rect[1] -= SHIFT;
        }
        if let Some(view) = &mut self.list_view {
            view[0] -= SHIFT;
            view[1] -= SHIFT;
        }
        let (w, h) = (sw, sh);
        if self.login.visible {
            self.list_targets.clear();
            self.list_view = None;
            self.login.draw(
                &mut ui,
                &mut self.targets,
                &self.catalog.config.name,
                self.catalog.editing == Some("name"),
            );
        }
        if let Some(chooser) = &mut self.chooser {
            chooser.draw(&mut ui, &mut self.targets);
            self.list_targets = chooser.navigation(ui.w);
            self.list_view = Some(chooser.viewport(ui.h));
        }
        if let Some([top, bottom, maximum]) = self.list_view
            && maximum > 0.0
            && bottom > top
        {
            let scroll = self
                .chooser
                .as_ref()
                .map_or(self.catalog.scroll, |c| c.scroll);
            let height = bottom - top;
            let thumb = (height * height / (height + maximum)).max(20.0).min(height);
            let y = top + (height - thumb) * (scroll / maximum).clamp(0.0, 1.0);
            ui.rounded(w - 12.0, top, 4.0, height, 2.0, color::BORDER);
            ui.rounded(w - 12.0, y, 4.0, thumb, 2.0, color::MUTED);
        }
        let caret_visible = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .is_ok_and(|t| t.as_millis() % 1000 < 500);
        if caret_visible && let Some(name) = self.catalog.editing {
            let value = match name {
                "name" => self.catalog.config.name.clone(),
                "server" => self.catalog.config.server.clone(),
                "render_scale" => self.catalog.render_scale_text.clone(),
                "github_repo" => self.catalog.config.github_repo.clone(),
                "github_token" => "*".repeat(self.catalog.github_token_text.chars().count()),
                _ => format!("Search: {}", self.catalog.query),
            };
            for ([x, y, w, _], action) in &self.targets {
                if *action == Action::Field(name)
                    || (name == "render_scale" && *action == Action::RenderScale)
                {
                    let text = ui.fit_text(Face::Medium, 16.0, &value, w - 24.0);
                    let caret_x = x + 12.0 + ui.measure(Face::Medium, 16.0, &text);
                    ui.rounded(
                        caret_x.min(x + w - 10.0),
                        y + 12.0,
                        1.0,
                        18.0,
                        0.0,
                        color::TEXT,
                    );
                }
            }
        }
        if self.exit_confirmation {
            self.targets.clear();
            self.list_targets.clear();
            self.list_view = None;
            ui.rounded(0.0, 0.0, w, h, 0.0, horizon_client::hud::rgba(0, 0, 0, 160));
            let width = (w - 32.0).min(360.0);
            let x = (w - width) * 0.5;
            let y = (h - 160.0) * 0.5;
            ui.rounded(x, y, width, 160.0, 24.0, color::RAISED);
            ui.text(
                Face::Medium,
                22.0,
                x + 20.0,
                y + 24.0,
                color::TEXT,
                "Exit application?",
            );
            let bw = (width - 52.0) * 0.5;
            button(
                &mut ui,
                &mut self.targets,
                [x + 20.0, y + 92.0, bw, 44.0],
                "Cancel",
                Action::Dismiss,
                false,
            );
            button(
                &mut ui,
                &mut self.targets,
                [x + 32.0 + bw, y + 92.0, bw, 44.0],
                "Exit",
                Action::ConfirmExit,
                false,
            );
        }
        if let Some(action) = self.pending_focus.take() {
            self.focus = self.targets.iter().position(|(_, a)| *a == action);
        }
        if let Some(index) = self.focus
            && let Some(([x, y, w, h], _)) = self.targets.get(index)
        {
            ui.border(
                *x - 2.0,
                *y - 2.0,
                *w + 4.0,
                *h + 4.0,
                (*h / 2.0 + 2.0).min(18.0),
                color::ACCENT,
            );
            self.cursor = (
                f64::from((x + w / 2.0) * ui.density),
                f64::from((y + h / 2.0) * ui.density),
            );
        }
        if self.mouse_down || self.drag.as_ref().is_some_and(|drag| !drag.moved) {
            let point = [
                self.cursor.0 as f32 / ui.density,
                self.cursor.1 as f32 / ui.density,
            ];
            if let Some(([x, y, w, h], _)) = self.targets.iter().rev().find(|(rect, _)| {
                point[0] >= rect[0]
                    && point[0] <= rect[0] + rect[2]
                    && point[1] >= rect[1]
                    && point[1] <= rect[1] + rect[3]
            }) {
                ui.rounded(
                    *x,
                    *y,
                    *w,
                    *h,
                    (*h / 2.0).min(16.0),
                    horizon_client::hud::rgba(255, 255, 255, 36),
                );
            }
        }
        if !self.mouse_down
            && self.drag.is_none()
            && let Some((action, time)) = self.feedback
            && time.elapsed() < std::time::Duration::from_millis(160)
            && let Some(([x, y, w, h], _)) = self.targets.iter().find(|(_, a)| *a == action)
        {
            ui.rounded(
                *x,
                *y,
                *w,
                *h,
                (*h / 2.0).min(16.0),
                horizon_client::hud::rgba(255, 255, 255, 36),
            );
        }
        crate::documents::draw_ui(&mut ui);
        ui.finish();

        if !r.render(&game::loading_frame(&b))? && !b.patches.is_empty() {
            // A suspended/lost surface skips the frame, including its glyph
            // uploads. Rebuild them next time instead of caching invisible text.
            horizon_client::ui_theme::reset_atlas();
        }
        Ok(())
    }
    pub(super) fn activate(&mut self, events: &ActiveEventLoop) -> Result<()> {
        if self.session_worker.is_some() && self.chooser.is_none() && !self.exit_confirmation {
            return Ok(());
        }
        let Some(r) = &self.renderer else {
            return Ok(());
        };
        let mut b = HudBatch::new([r.config.width, r.config.height]);
        let ui = Ui::mobile(&self.gui.as_ref().unwrap().atlas, &mut b, (0.0, 0.0));
        let p = self.focus.and_then(|index| self.targets.get(index)).map_or(
            [
                self.cursor.0 as f32 / ui.density,
                self.cursor.1 as f32 / ui.density,
            ],
            |([x, y, w, h], _)| [x + w / 2.0, y + h / 2.0],
        );
        let action = self
            .targets
            .iter()
            .find(|([x, y, w, h], _)| {
                (*x..*x + *w).contains(&p[0]) && (*y..*y + *h).contains(&p[1])
            })
            .map(|(_, a)| *a);
        if self.catalog.busy() || crate::documents::busy() {
            return Ok(());
        }
        self.feedback = action.map(|action| (action, std::time::Instant::now()));
        self.focus = None;
        if action.is_some() && horizon_client::official_install::ready(&self.home) {
            let audio = self.audio.get_or_insert_with(|| {
                let mut audio = horizon_client::audio::Audio::new(
                    &self.home.join(".local/horizon-client/26.3"),
                );
                let settings = horizon_client::settings::Settings::load(
                    &self
                        .catalog
                        .config
                        .game_home(&self.home)
                        .join("options.txt"),
                );
                audio.master = settings.volumes[0];
                let mut volumes = settings.volumes;
                volumes[0] = 1.0;
                horizon_client::settings::set_source_volumes(volumes);
                audio
            });
            audio.play_button_click();
        }
        match action {
            Some(Action::ConfirmExit) => {
                events.exit();
            }
            Some(Action::Immersive) => {
                self.catalog.config.screen.immersive ^= true;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
            }
            Some(Action::Cutout) => {
                self.catalog.config.screen.avoid_cutout ^= true;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
            }
            Some(Action::Aspect) => self.open_chooser(chooser::Kind::Aspect),
            Some(Action::ChooseAspect(aspect)) => {
                self.catalog.config.screen.aspect = aspect;
                self.catalog.config.save(&self.home)?;
                self.configure_screen();
                self.chooser = None;
            }
            Some(Action::CacheBudget) => self.open_chooser(chooser::Kind::CacheBudget),
            Some(Action::ChooseCacheBudget(mib)) => {
                self.catalog.config.cache_budget_mib = mib.min(1024);
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                self.catalog.status = "Disposable cache budget saved. Applies on next Play; not a total RAM or VRAM limit.".into();
            }
            Some(Action::ToggleBundled(id)) => {
                let game = self.catalog.config.game_home(&self.home);
                let mut list = horizon_client::mod_list::ModList::load(&game);
                let enabled = list.enabled.contains(id) && !list.disabled.contains(id);
                if enabled {
                    list.enabled.remove(id);
                    list.disabled.insert(id.into());
                } else {
                    list.disabled.remove(id);
                    list.enabled.insert(id.into());
                }
                std::fs::create_dir_all(&game)?;
                std::fs::write(game.join(horizon_client::mod_list::FILE), list.serialize())?;
                self.catalog.refresh(&self.home);
                self.catalog.status = format!(
                    "Bundled {id} {}. Applies on Play; settings are in Options > Mods.",
                    if enabled { "disabled" } else { "enabled" }
                );
            }
            Some(Action::GameFiles) => {
                self.catalog.status = if crate::platform::open_documents(crate::document_store::ROOT) {
                    "Game files opened. Worlds, mods, packs, shaders and reports are available here."
                } else { "Open system Files > Horizon Android." }.into();
            }
            Some(Action::WorldFiles) => {
                let id = crate::document_store::folder_id(self.catalog.config.profile, "worlds");
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Copy a complete world folder to back up or import. Finish copying before Play; never replace an existing world silently."
                } else { "Open system Files > Horizon Android > Worlds. Copy complete world folders while the game is closed." }.into();
            }
            Some(Action::WorkerLimit) => self.open_chooser(chooser::Kind::WorkerLimit),
            Some(Action::ChooseWorkerLimit(limit)) => {
                self.catalog.config.worker_limit = limit.min(64);
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                self.catalog.status = "Chunk worker ceiling saved. Applies on the next Play; other engine threads are unchanged.".into();
            }
            Some(Action::DisplayHz) => self.open_chooser(chooser::Kind::DisplayHz),
            Some(Action::ChooseDisplayHz(hz)) => {
                self.catalog.config.device.display_hz = hz;
                self.catalog.config.device.sanitize();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
                self.chooser = None;
            }
            Some(Action::PerformanceHints) => {
                self.catalog.config.device.performance_hints ^= true;
                self.catalog.status="ADPF gives Android CPU frame-work hints on supported devices. It does not cap FPS or guarantee a speedup.".into();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
            }
            Some(Action::GpuProfile) => {
                self.catalog.config.device.gpu_profile ^= true;
                self.catalog.status="Detailed GPU profiling records diagnostics and has overhead. Reports remain on this device.".into();
                self.catalog.config.save(&self.home)?;
                crate::device_runtime::configure(self.catalog.config.device);
            }

            Some(Action::ModSource(origin)) => {
                self.catalog.config.mod_source = origin;
                self.catalog.config.save(&self.home)?;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::ShaderSource(origin)) => {
                self.catalog.config.shader_source = origin;
                self.catalog.config.save(&self.home)?;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::OpenFolder(kind)) => {
                crate::platform::hide();
                let folder = match kind {
                    horizon_client::content_files::Kind::Mods => "mods",
                    horizon_client::content_files::Kind::NativeMods => "native-mods",
                    horizon_client::content_files::Kind::Packs => "packs",
                    horizon_client::content_files::Kind::Shaders => "shaders",
                };
                let id = crate::document_store::folder_id(self.catalog.config.profile, folder);
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Opened in Files. Return here after copying files.".into()
                } else {
                    "Open the system Files app and choose Horizon Android.".into()
                };
            }
            Some(Action::Import(kind)) => {
                crate::platform::hide();
                let home = self.catalog.config.game_home(&self.home);
                if let Err(e) = crate::documents::begin(kind, home) {
                    self.catalog.status = e;
                }
            }
            Some(Action::ToggleSection(i)) => {
                self.catalog.sections[i] = !self.catalog.sections[i];
                self.catalog.scroll = 0.0;
            }
            Some(Action::ToggleLocal(i)) => self.catalog.toggle_local(&self.home, i),
            Some(Action::DeleteLocal(i)) => {
                self.catalog.pending_delete = self.catalog.local.get(i).cloned();
                if let Some(item) = &self.catalog.pending_delete {
                    self.chooser = Some(super::chooser::Chooser::confirm_delete(item.file.clone()));
                }
            }
            Some(Action::BuildNative) => {
                let repo = crate::native_build::repository(&self.catalog.config.github_repo);
                if repo.is_none() || !self.catalog.github_token_saved {
                    // Not set up: the steps on a screen of their own, not the status line.
                    self.chooser = Some(super::chooser::Chooser::setup_build(
                        repo,
                        self.catalog.github_token_saved,
                        None,
                    ));
                } else {
                    match self.catalog.native_build_inputs(&self.home) {
                        Ok((repo, _, mods)) => {
                            self.chooser =
                                Some(super::chooser::Chooser::confirm_build(mods.len(), repo));
                        }
                        Err(e) => self.catalog.status = e,
                    }
                }
            }
            Some(Action::OpenTokenPage) => {
                let repo = crate::native_build::repository(&self.catalog.config.github_repo);
                crate::platform::open_url(&crate::native_build_plan::token_page(repo.as_deref()));
            }
            Some(Action::PasteToken) => {
                let pasted = crate::platform::clipboard_get();
                let note = match pasted.as_deref().and_then(crate::native_build_plan::token) {
                    Some(token) => match crate::native_build::save_token(token) {
                        Ok(()) => {
                            self.catalog.github_token_saved = true;
                            "Token saved, encrypted on this phone. Now tap Build APK.".to_owned()
                        }
                        Err(e) => format!("Could not save the token: {e}"),
                    },
                    None => "The clipboard has no GitHub token: copy it on GitHub first".into(),
                };
                self.chooser = Some(super::chooser::Chooser::setup_build(
                    crate::native_build::repository(&self.catalog.config.github_repo),
                    self.catalog.github_token_saved,
                    Some(note),
                ));
            }
            Some(Action::ConfirmBuildNative) => {
                self.chooser = None;
                self.catalog.build_native(&self.home);
            }
            Some(Action::InstallNative) => {
                let id = format!(
                    "{}/{}",
                    crate::document_store::folder_id(self.catalog.config.profile, "builds"),
                    crate::native_build::APK_FILE
                );
                self.catalog.status = if crate::platform::install_package(&id) {
                    "Android's installer opened. Allow Horizon to install apps if it asks.".into()
                } else {
                    "Could not open the installer: install the APK from Files > APK builds".into()
                };
            }
            Some(Action::OpenNativeRun) => {
                if let Some(url) = self.catalog.native_build.url() {
                    crate::platform::open_url(&url);
                }
            }
            Some(Action::ConfirmDeleteLocal) => {
                self.catalog.remove_local(&self.home);
                self.chooser = None;
            }
            Some(Action::Repository) => {
                self.catalog.status = if crate::platform::open_repository() {
                    "Repository opened in browser."
                } else {
                    "No browser available to open the repository."
                }
                .into();
            }
            Some(Action::Account) => {
                self.login.visible = true;
            }
            Some(Action::CloseLogin) => {
                self.login.hide();
                crate::platform::hide();
                self.catalog.editing = None;
            }
            Some(Action::DeviceSignIn) => {
                self.session_ready = false;
                self.prepared_session = None;
                self.login
                    .start_device(&self.catalog.config.microsoft_client_id);
            }
            Some(Action::CopyLoginCode) => self.login.copy_code(),
            Some(Action::CancelLogin) => {
                self.login.close();
                self.login.visible = true;
            }
            Some(Action::ReviewApp) => self.login.review_app(),
            Some(Action::SignIn) => {
                self.session_ready = false;
                self.prepared_session = None;
                self.login.start(&self.catalog.config.microsoft_client_id);
            }
            Some(Action::SignOut) => {
                self.session_ready = false;
                self.prepared_session = None;
                self.login.sign_out();
            }
            Some(Action::LoginLink) => self.login.open_link(),
            Some(Action::Page(page)) => {
                self.chooser = None;
                self.catalog.editing = None;
                if crate::platform::active() {
                    crate::platform::hide();
                }
                self.catalog.page = page;
                self.catalog.scroll = 0.0;
                self.error = None;
                self.catalog.offset = 0;
                self.catalog.browse(&self.home);
            }
            Some(Action::Play) => self.action(events)?,
            Some(Action::Search) => {
                crate::platform::hide();
                self.catalog.offset = 0;
                self.catalog.search(&self.home);
            }
            Some(Action::FinishNameEdit) => {
                self.catalog.editing = None;
                crate::platform::hide();
                self.catalog.config.save(&self.home)?;
            }
            Some(Action::Field(name)) => {
                if name == "render_scale" {
                    self.catalog.render_scale_text =
                        self.catalog.config.render_scale.percent().to_string();
                    self.chooser = None;
                }
                self.catalog.editing = Some(name);
                let initial = match name {
                    "name" => &self.catalog.config.name,
                    "server" => &self.catalog.config.server,
                    "render_scale" => &self.catalog.render_scale_text,
                    "client_id" => &self.catalog.config.microsoft_client_id,
                    "github_repo" => &self.catalog.config.github_repo,
                    "github_token" => {
                        self.catalog.github_token_text.clear();
                        &self.catalog.github_token_text
                    }
                    _ => &self.catalog.query,
                };
                crate::platform::show(initial);
            }
            Some(Action::Install(i)) => self.catalog.install(&self.home, i),
            Some(Action::Next) => {
                self.catalog.offset += 12;
                self.catalog.search(&self.home);
            }
            Some(Action::Previous) => {
                self.catalog.offset = self.catalog.offset.saturating_sub(12);
                self.catalog.search(&self.home);
            }
            Some(Action::Profile) => self.open_chooser(chooser::Kind::Profile),
            Some(Action::Renderer) => self.open_chooser(chooser::Kind::Renderer),
            Some(Action::RenderScale) => self.open_chooser(chooser::Kind::RenderScale),
            Some(Action::ChooseRenderScale(scale)) => {
                self.catalog.config.render_scale = scale;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::display::set_scale(scale);
            }
            Some(Action::Orientation) => self.open_chooser(chooser::Kind::Orientation),
            Some(Action::InputHost) => self.open_chooser(chooser::Kind::InputHost),
            Some(Action::ChooseInputHost(mode)) => {
                self.catalog.config.input_host = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::configure_input_mode(mode);
            }
            Some(Action::ChooseOrientation(mode)) => {
                self.catalog.config.orientation = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
                crate::platform::set_orientation(mode);
            }
            Some(Action::ChooseRenderer(mode)) => {
                self.catalog.config.renderer = mode;
                self.catalog.config.save(&self.home)?;
                self.chooser = None;
            }
            Some(Action::ChooseProfile(profile)) => {
                std::fs::create_dir_all(if profile == 0 {
                    self.home.clone()
                } else {
                    self.home
                        .join("profiles")
                        .join(format!("profile-{profile}"))
                })?;
                self.catalog.config.profile = profile;
                self.catalog.config.save(&self.home)?;
                self.catalog.refresh(&self.home);
                self.chooser = None;
            }
            Some(Action::Dismiss) => {
                self.exit_confirmation = false;
                self.chooser = None;
                if self.session_worker.is_none() {
                    self.session_ready = false;
                    self.prepared_session = None;
                }
            }
            Some(Action::Logs) => {
                self.catalog.status = crate::diagnostics::summary(&self.home);
                if let Ok(error) = std::fs::read_to_string(self.home.join("android-error.txt")) {
                    self.catalog.status = format!(
                        "Last handled error: {}. {}",
                        crate::diagnostics::redact(&error),
                        self.catalog.status
                    );
                }
            }
            Some(Action::ExportDiagnostics) if self.diagnostic_export.is_none() => {
                let home = self.home.clone();
                let (sender, receiver) = mpsc::channel();
                self.diagnostic_export = Some(receiver);
                self.catalog.status =
                    "Preparing local diagnostic ZIP. No upload; review before sharing.".into();
                std::thread::spawn(move || {
                    let result = crate::diagnostics::export(&home).map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    });
                    let _ = sender.send(result);
                });
            }
            Some(Action::ExportDiagnostics) => {}
            Some(Action::RecordPerformance) => {
                self.catalog.config.record_performance = !self.catalog.config.record_performance;
                if !self.catalog.config.record_performance {
                    crate::performance::stop();
                }
                self.catalog.config.save(&self.home)?;
                self.catalog.status = if self.catalog.config.record_performance {
                    "Recording starts on Play. Detailed timings add overhead. Return here to save the report."
                } else { "Performance recording off." }.into();
            }
            Some(Action::ExportPerformance) => {
                self.catalog.status = match crate::performance::export(&self.home) {
                    Ok(()) => "Reports opened in Files. Copy or share a report from there.".into(),
                    Err(e) => e,
                };
            }
            None => {}
        }
        Ok(())
    }
    pub(super) fn ui_density(&self) -> f32 {
        let (Some(r), Some(gui)) = (&self.renderer, &self.gui) else {
            return 1.0;
        };
        let mut batch = HudBatch::new([r.config.width, r.config.height]);
        Ui::mobile(&gui.atlas, &mut batch, (0.0, 0.0)).density
    }

    pub(super) fn touch_event(&mut self, events: &ActiveEventLoop, t: Touch) -> Result<()> {
        self.focus = None;
        self.pending_focus = None;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        self.cursor = (t.location.x, t.location.y);
        match t.phase {
            TouchPhase::Started => {
                self.drag = Some(super::gesture::Drag::new(t.location.x, t.location.y));
            }
            TouchPhase::Moved => {
                let density = self.ui_density() as f64;
                if let Some(drag) = &mut self.drag {
                    let delta = drag.update(t.location.x, t.location.y, density);
                    if drag.moved {
                        if let Some(chooser) = &mut self.chooser {
                            chooser.scroll = (chooser.scroll + delta).max(0.0);
                        } else if self
                            .list_view
                            .is_some_and(|view| drag.origin_y() / density >= view[0] as f64)
                        {
                            self.catalog.scroll = (self.catalog.scroll + delta).max(0.0);
                        }
                    }
                }
            }
            TouchPhase::Ended => {
                if self.drag.take().is_some_and(|drag| !drag.moved) {
                    self.activate(events)?;
                }
            }
            TouchPhase::Cancelled => {
                self.drag = None;
            }
        }
        Ok(())
    }
    pub(super) fn type_key(&mut self, key: KeyEvent) {
        if key.physical_key == PhysicalKey::Code(KeyCode::Escape)
            || key.logical_key
                == winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserBack)
        {
            if !key.repeat {
                self.request_back();
            }
            return;
        }
        if self.catalog.editing.is_none() {
            return;
        }
        let out = match key.physical_key {
            PhysicalKey::Code(KeyCode::Backspace) => Some(Output::Backspace),
            PhysicalKey::Code(KeyCode::Enter) => {
                crate::platform::hide();
                Some(Output::Enter)
            }
            _ => key.text.map(|s| Output::Text(s.into())),
        };
        if let Some(out) = out {
            self.catalog.edit(out, &self.home);
        }
    }
}
