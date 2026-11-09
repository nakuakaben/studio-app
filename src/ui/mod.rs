mod catalog_state;
mod gallery;
mod theme;
use crate::catalog::{loader::Origin, worker::Event};
use std::{collections::HashMap, sync::mpsc::Receiver};

use crate::{
    demo,
    domain::{AppState, Application, Operation},
};
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke};
use theme::{accent, mono, BG, BLUE, WHITE};

use crate::preferences::{Appearance, Language, Preferences, Sort, View};
mod library;
mod native_state;
mod settings;

pub struct StudioApp {
    apps: Vec<Application>,
    native_generation: u64,
    native_pending: bool,
    native_job: Option<Receiver<crate::native::worker::Snapshot>>,
    launch_job: Option<Receiver<(String, Result<(), String>)>>,
    native_status: String,
    query: String,
    sort: Sort,
    preferences: Preferences,
    settings_open: bool,
    preferences_path: Option<std::path::PathBuf>,
    selected: Option<usize>,
    slide: usize,
    notice: Option<String>,
    catalog_status: String,
    catalog_origin: Option<Origin>,
    job: Option<Receiver<Event>>,
    images: HashMap<String, egui::TextureHandle>,
    asset_errors: HashMap<String, String>,
}

impl StudioApp {
    /// State selection for the opt-in native screenshot harness only.
    #[cfg(feature = "visual-qa")]
    pub fn prepare_visual_qa(&mut self, state: &str) {
        self.preferences_path = None;
        self.preferences.appearance = if state.contains("light") {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        if state.contains("settings") {
            self.settings_open = true;
        }
        if state.contains("list") {
            self.preferences.view = View::List;
        }
        if state.contains("detail") && !self.apps.is_empty() {
            self.select(0);
        }
    }

    pub fn new(ctx: &egui::Context) -> Self {
        let mut app = Self::empty(ctx);
        app.refresh_catalog(ctx);
        app
    }

    fn empty(ctx: &egui::Context) -> Self {
        theme::apply(ctx);
        let preferences_path = Preferences::path();
        let (preferences, warning) = match preferences_path
            .as_ref()
            .filter(|p| p.exists())
            .map(|p| Preferences::load(p))
        {
            Some(Ok(p)) => (p, None),
            Some(Err(e)) => (
                Preferences::default(),
                Some(format!(
                    "Préférences invalides : {e}. Valeurs par défaut utilisées."
                )),
            ),
            None => (Preferences::default(), None),
        };
        Self {
            apps: Vec::new(),
            native_generation: 0,
            native_pending: false,
            native_job: None,
            launch_job: None,
            native_status: String::new(),
            query: String::new(),
            sort: preferences.sort,
            preferences,
            settings_open: false,
            preferences_path,
            selected: None,
            slide: 0,
            notice: warning,
            catalog_status: "Catalogue indisponible".into(),
            catalog_origin: None,
            job: None,
            images: HashMap::new(),
            asset_errors: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn test_app(ctx: &egui::Context) -> Self {
        let mut app = Self::empty(ctx);
        app.preferences = Preferences::default();
        app.preferences_path = None;
        app.sort = Sort::Name;
        app.apps = crate::catalog::demonstration();
        app.catalog_origin = Some(Origin::Local);
        app.catalog_status = "Catalogue de test".into();
        app
    }

    fn visible(&self) -> Vec<usize> {
        let q = self.query.trim().to_lowercase();
        let mut indices: Vec<_> = self
            .apps
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                format!("{} {}", a.name, a.category)
                    .to_lowercase()
                    .contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| match self.sort {
            Sort::Name => self.apps[i].name.as_str(),
            Sort::Category => self.apps[i].category.as_str(),
        });
        indices
    }

    fn select(&mut self, index: usize) {
        self.selected = Some(index);
        self.slide = 0;
    }

    fn navigate(&mut self, forward: bool) {
        let visible = self.visible();
        if let Some(position) = visible.iter().position(|i| Some(*i) == self.selected) {
            let next = if forward {
                (position + 1) % visible.len()
            } else {
                (position + visible.len() - 1) % visible.len()
            };
            self.select(visible[next]);
        }
    }

    fn action(&mut self, index: usize) {
        if !self.apps[index].fictional {
            self.open_application(index);
        } else if self.apps[index].state() == AppState::Installed {
            self.notice = Some(format!(
                "Ouverture simulée de {}. Aucun programme n’a été lancé.",
                self.apps[index].name
            ));
        } else {
            demo::start(&mut self.apps[index]);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.apply_appearance(ctx);
        self.poll_catalog(ctx);
        self.poll_native(ctx);
        let dt = ctx.input(|i| i.stable_dt);
        for app in &mut self.apps {
            demo::tick(app, dt);
        }
        if self.apps.iter().any(|a| {
            matches!(
                a.state(),
                AppState::Downloading { paused: false, .. } | AppState::Installing { .. }
            )
        }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if self.selected.is_some() && !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.navigate(true);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                self.navigate(false);
            }
        }
        self.library(ctx);
        if let Some(index) = self.selected {
            self.details(ctx, index);
        }
        if self.settings_open {
            self.settings(ctx);
        }
    }

    fn details(&mut self, ctx: &egui::Context, index: usize) {
        let (bg, border, muted, blue) = theme::palette(ctx);
        let mut close = false;
        let mut previous = false;
        let mut next = false;
        let mut action = false;
        let app = &self.apps[index];
        let color = accent(app.color);
        let size = ctx.content_rect().size();
        let modal = egui::Modal::new(egui::Id::new("application_details")).frame(egui::Frame::new().fill(bg).stroke(Stroke::new(1.0_f32,border))).show(ctx, |ui| {
            ui.set_width((size.x - 64.0).clamp(320.0, 1140.0));
            ui.set_height(size.y - 100.0);
            let (line,_) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), Sense::hover());
            ui.painter().rect_filled(line, 0, color);
            egui::ScrollArea::vertical().id_salt(("detail_scroll", &app.id)).max_height(size.y - 100.0).show(ui, |ui| {
                egui::Frame::new().fill(theme::tinted(ctx, app.color)).inner_margin(24).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(mono(if size.x < 650.0 { "APPLICATION" } else { if app.fictional { "DÉTAILS / APPLICATION FICTIVE" } else { "DÉTAILS DE L’APPLICATION" } }));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            close = ui.button("×").on_hover_text("Fermer · Échap").clicked();
                            next = ui.button("›").on_hover_text("Application suivante · →").clicked();
                            previous = ui.button("‹").on_hover_text("Application précédente · ←").clicked();
                        });
                    });
                    ui.add_space(14.0);
                    ui.horizontal_top(|ui| {
                        logo(ui, app, if size.x < 650.0 {64.0} else {98.0}, &self.images);
                        ui.vertical(|ui| {
                            ui.heading(theme::title(&app.name,34.0));
                            ui.label(&app.category);
                            if let Some(notice) = &self.notice { ui.colored_label(blue, notice); }
                            ui.label(RichText::new(match app.displayed_installed_version() { Some(v) => format!("{v} {} / {} disponible", if app.simulation.version.is_some() {"simulée"} else {"locale détectée"}, app.available_version()), None if !app.local.installations.is_empty()=>format!("Version locale inconnue / {} disponible",app.available_version()), None => format!("Version disponible {}", app.available_version()) }).color(muted).size(13.0));
                        });
                    });
                    ui.add_space(16.0);
                    ui.horizontal_wrapped(|ui| {
                        action = ui.add_enabled_ui(!app.state().busy() && (app.fictional || !app.local.installations.is_empty()), |ui| primary(ui, if app.fictional {app.state().action()} else {"OUVRIR"}, 260.0)).inner.clicked();
                        for (label, link) in [("SITE WEB ↗", &app.links.website), ("GITHUB ↗", &app.links.github), ("DOCUMENTATION ↗", &app.links.documentation)] {
                            if let Some(link) = link { ui.hyperlink_to(label, link); }
                        }
                    });
                    ui.label(mono(if app.fictional {"SIMULATION LOCALE · AUCUN LOGICIEL N’EST TÉLÉCHARGÉ OU EXÉCUTÉ"}else{"OUVERTURE NATIVE · AUCUNE INSTALLATION NI MISE À JOUR"}).size(10.0));
                });
                egui::Frame::new().inner_margin(24).show(ui, |ui| {
                    ui.label(&app.description);
                    if let AppState::Error { message } = &app.state() { ui.colored_label(Color32::LIGHT_RED, message); }
                    if let AppState::Downloading { progress, .. } | AppState::Installing { progress } = app.state() {
                        ui.label(mono(app.state().label()).color(color));
                        ui.add(egui::ProgressBar::new(progress).fill(color).show_percentage());
                    }
                    ui.add_space(18.0);
                    if ui.available_width() >= 760.0 {
                        let width = ui.available_width();
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(width*0.56-12.0,0.0), Layout::top_down(Align::Min), |ui| {
                                self_gallery(ui, app, &mut self.slide, &self.images, &self.asset_errors);
                                ui.add_space(20.0); release(ui, app);
                            });
                            ui.add_space(14.0);
                            ui.allocate_ui_with_layout(egui::vec2(width*0.44-22.0,0.0), Layout::top_down(Align::Min), |ui| { features(ui, app); });
                        });
                    } else {
                        self_gallery(ui, app, &mut self.slide, &self.images, &self.asset_errors);
                        ui.add_space(20.0); features(ui, app);
                        ui.add_space(20.0); release(ui, app);
                    }
                });
            });
        });
        if close || modal.should_close() {
            self.selected = None;
        } else if previous {
            self.navigate(false);
        } else if next {
            self.navigate(true);
        }
        if action {
            self.action(index);
        }
    }
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}

fn primary(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 38.0],
        egui::Button::new(mono(text).size(11.0).strong().color(BG)).fill(WHITE),
    )
}

fn logo(
    ui: &mut egui::Ui,
    app: &Application,
    side: f32,
    images: &HashMap<String, egui::TextureHandle>,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), Sense::hover());
    let p = ui.painter();
    if let Some(texture) = app.logo.as_ref().and_then(|a| images.get(&a.source)) {
        gallery::paint_image(p, rect, texture);
        return;
    }
    p.rect_filled(rect, (side * 0.2) as u8, accent(app.color));
    p.line_segment(
        [
            rect.left_bottom() + egui::vec2(side * 0.15, -side * 0.12),
            rect.right_top() + egui::vec2(-side * 0.15, side * 0.12),
        ],
        Stroke::new(1.0_f32, Color32::from_black_alpha(45)),
    );
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &app.monogram,
        egui::FontId::proportional(side * 0.43),
        BG,
    );
}

fn self_gallery(
    ui: &mut egui::Ui,
    app: &Application,
    slide: &mut usize,
    images: &HashMap<String, egui::TextureHandle>,
    errors: &HashMap<String, String>,
) {
    let count = if app.screenshots.is_empty() {
        if app.fictional {
            3
        } else {
            1
        }
    } else {
        app.screenshots.len()
    };
    *slide %= count;
    gallery::preview(ui, app, *slide, ui.available_width(), images);
    ui.horizontal(|ui| {
        if ui.button("‹").clicked() {
            *slide = (*slide + count - 1) % count;
        }
        let caption = app
            .screenshots
            .get(*slide)
            .map(|s| s.caption.as_str())
            .unwrap_or(if app.fictional {
                gallery::CAPTIONS[*slide]
            } else {
                "AUCUNE CAPTURE FOURNIE"
            });
        ui.label(mono(format!("{caption} · {}/{count}", *slide + 1)).size(10.0));
        if ui.button("›").clicked() {
            *slide = (*slide + 1) % count;
        }
    });
    let width = (ui.available_width() - 20.0) / 3.0;
    ui.horizontal_wrapped(|ui| {
        for i in 0..count {
            let r = gallery::preview(ui, app, i, width, images);
            if r.clicked() {
                *slide = i;
            }
            if i == *slide {
                ui.painter().rect_stroke(
                    r.rect,
                    0,
                    Stroke::new(2.0_f32, accent(app.color)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    });
    if app.fictional && app.screenshots.is_empty() {
        ui.label(mono("APERÇUS CONCEPTUELS · APPLICATION FICTIVE").size(9.0));
    }
    for asset in app
        .logo
        .iter()
        .chain(app.screenshots.iter().map(|s| &s.image))
    {
        if let Some(error) = errors.get(&asset.source) {
            ui.label(mono("Ressource indisponible").color(BLUE))
                .on_hover_text(error);
        }
    }
}

fn features(ui: &mut egui::Ui, app: &Application) {
    ui.label(mono("01 / FONCTIONNALITÉS ENVISAGÉES"));
    ui.add_space(10.0);
    for (i, feature) in app.features.iter().enumerate() {
        theme::outline(ui.ctx()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(mono(format!("{:02}", i + 1)).color(accent(app.color)));
                ui.add_space(10.0);
                ui.strong(&feature.title);
            });
            ui.label(
                RichText::new(&feature.description)
                    .color(theme::palette(ui.ctx()).2)
                    .size(14.0),
            );
        });
    }
}

fn release(ui: &mut egui::Ui, app: &Application) {
    ui.label(mono("02 / VERSION DU CATALOGUE"));
    ui.add_space(10.0);
    theme::outline(ui.ctx()).show(ui, |ui| {
        ui.label(mono("VERSION DISPONIBLE"));
        ui.label(app.available_version());
        ui.label(mono("VERSION LOCALE DÉTECTÉE"));
        ui.label(
            app.local
                .version
                .as_deref()
                .unwrap_or(if app.local.installations.is_empty() {
                    "Non détectée"
                } else {
                    "Inconnue"
                }),
        );
        for (i, installation) in app.local.installations.iter().enumerate() {
            ui.label(mono(if i == 0 {
                "INSTALLATION RETENUE"
            } else {
                "AUTRE INSTALLATION DÉTECTÉE"
            }));
            ui.label(installation.path.display().to_string());
        }
        for error in &app.local.errors {
            ui.label(RichText::new(error).size(12.0));
        }
        if let Some(version) = &app.simulation.version {
            ui.label(mono("VERSION SIMULÉE"));
            ui.label(version);
        }
        ui.label(mono("TAILLE / PLATEFORMES CIBLES"));
        ui.label(if app.release.packages.is_empty() {
            "Aucun package approuvé".to_owned()
        } else {
            format!("{} package(s) approuvé(s)", app.release.packages.len())
        });
        for platform in &app.platforms {
            ui.label(format!("{:?} / {:?}", platform.os, platform.architectures));
        }
        ui.label(mono("ÉDITEUR"));
        ui.label(&app.publisher);
    });
    ui.add_space(8.0);
    ui.strong("Notes de version du catalogue");
    for previous in &app.history {
        ui.collapsing(
            format!("Version {} · {}", previous.version, previous.published),
            |ui| {
                for note in &previous.notes {
                    ui.label(note);
                }
            },
        );
    }
    for note in &app.release.notes {
        ui.label(
            RichText::new(format!("· {note}"))
                .color(theme::palette(ui.ctx()).2)
                .size(14.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        app: &mut StudioApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 960.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.show(ctx),
        )
    }

    fn click_text(app: &mut StudioApp, ctx: &egui::Context, text: &str) {
        let mut output = frame(app, ctx, vec![]);
        for _ in 0..24 {
            output = frame(app, ctx, vec![]);
        }
        let position = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape {
                    if t.galley.job.text == text {
                        return Some(t.pos + t.galley.size() * 0.5);
                    }
                }
                None
            })
            .unwrap_or_else(|| panic!("Text not rendered: {text}"));
        frame(app, ctx, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }

    fn text_positions(output: &egui::FullOutput, text: &str) -> Vec<egui::Pos2> {
        output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == text).then_some(t.pos)
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn five_joint_columns_and_button_bounds_at_reference_and_hidpi() {
        for scale in [1.0, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let mut app = StudioApp::test_app(&ctx);
            let metadata = app.apps[0].metadata.clone();
            app.apps = (0..6)
                .map(|i| {
                    let mut m = metadata.clone();
                    m.id = format!("app-{i}");
                    m.name = format!("App {i}");
                    Application::new(m)
                })
                .collect();
            let mut output = frame(&mut app, &ctx, vec![]);
            for _ in 0..3 {
                output = frame(&mut app, &ctx, vec![]);
            }
            let positions: Vec<_> = (0..6)
                .map(|i| text_positions(&output, &format!("App {i}"))[0])
                .collect();
            for pair in positions[..5].windows(2) {
                assert!((pair[1].y - pair[0].y).abs() < 1.0);
                assert!((pair[1].x - pair[0].x - 278.4).abs() < 3.0);
            }
            assert!((positions[5].y - positions[0].y - 306.0).abs() < 2.0);
            let buttons = text_positions(&output, "INSTALLER");
            assert_eq!(buttons.len(), 6);
            assert!(buttons.iter().all(|p| p.x > 24.0 && p.x < 1416.0));
            assert_eq!(library::columns(392.0), 1);
            assert_eq!(library::columns(752.0), 2);
            assert_eq!(library::columns(1616.0), 5);
            assert_eq!(library::columns(2500.0), 5);
        }
    }
    #[test]
    fn narrow_toolbar_and_cards_remain_inside_viewport() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        let mut output = None;
        for _ in 0..4 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(440.0, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| app.show(ctx),
            ));
        }
        let output = output.unwrap();
        for label in ["Nom (A–Z)", "TOUT METTRE À JOUR (1)", "OUVRIR"] {
            for shape in &output.shapes {
                if let egui::Shape::Text(t) = &shape.shape {
                    if t.galley.job.text == label {
                        assert!(
                            t.pos.x >= 0.0 && t.pos.x + t.galley.size().x <= 440.0,
                            "{label} overflow"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn hover_never_starts_operation_and_list_uses_same_actions() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        let out = frame(&mut app, &ctx, vec![]);
        let pos = text_positions(&out, "Sequence")[0];
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        assert_eq!(app.apps[0].operation, Operation::Idle);
        assert_eq!(app.selected, None);
        let visible = app.visible();
        click_text(&mut app, &ctx, "☰");
        assert_eq!(app.preferences.view, View::List);
        assert_eq!(app.visible(), visible);
        click_text(&mut app, &ctx, "METTRE À JOUR");
        assert!(matches!(
            app.apps[0].operation,
            Operation::SimulatedDownload { .. }
        ));
        assert_eq!(app.selected, None);
        app.query = "vidéo".into();
        assert_eq!(app.visible(), vec![0]);
        app.sort = Sort::Category;
        assert_eq!(app.visible(), vec![0]);
    }
    #[test]
    fn actual_local_metadata_is_never_labelled_as_simulated() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        app.apps.truncate(1);
        app.apps[0].simulation.version = None;
        app.apps[0].local.version = Some("1.6.0".into());
        let output = frame(&mut app, &ctx, vec![]);
        assert_eq!(
            text_positions(&output, "APPLICATIONS INSTALLÉES (1)").len(),
            1
        );
        app.select(0);
        let mut output = frame(&mut app, &ctx, vec![]);
        for _ in 0..12 {
            output = frame(&mut app, &ctx, vec![]);
        }
        assert_eq!(
            text_positions(&output, "1.6.0 locale détectée / 1.8.0 disponible").len(),
            1
        );
    }
    #[test]
    fn settings_pointer_theme_persistence_and_escape() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.json");
        app.preferences_path = Some(path.clone());
        click_text(&mut app, &ctx, "⚙");
        assert!(app.settings_open);
        click_text(&mut app, &ctx, "CLAIR");
        assert_eq!(
            app.preferences.appearance,
            Appearance::Light,
            "settings_open={} content={:?}",
            app.settings_open,
            ctx.content_rect()
        );
        assert!(!ctx.style().visuals.dark_mode);
        click_text(&mut app, &ctx, "TERMINÉ");
        assert!(!app.settings_open);
        assert_eq!(
            Preferences::load(&path).unwrap().appearance,
            Appearance::Light
        );
        click_text(&mut app, &ctx, "☰");
        assert_eq!(Preferences::load(&path).unwrap().view, View::List);
        click_text(&mut app, &ctx, "⚙");
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!app.settings_open);
        app.preferences = Preferences::load(&path).unwrap();
        assert_eq!(app.preferences.view, View::List);
    }
    #[test]
    fn actual_pointer_clicks_trigger_actions_and_open_details() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        click_text(&mut app, &ctx, "METTRE À JOUR");
        assert!(matches!(app.apps[0].state(), AppState::Downloading { .. }));
        assert_eq!(
            app.selected, None,
            "action must not open the enclosing card"
        );
        click_text(&mut app, &ctx, "Sequence");
        assert_eq!(app.selected, Some(0));
        click_text(&mut app, &ctx, "›");
        assert_eq!(app.selected, Some(5));
    }
    #[test]
    fn search_sort_and_navigation_follow_visible_catalog() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        app.query = "vidéo".into();
        assert_eq!(app.visible(), vec![0]);
        app.select(0);
        app.navigate(true);
        assert_eq!(app.selected, Some(0));
        app.query.clear();
        app.sort = Sort::Name;
        app.select(0);
        app.navigate(true);
        assert_eq!(app.apps[app.selected.unwrap()].name, "Trame");
        app.navigate(false);
        assert_eq!(app.selected, Some(0));
        app.query = "inexistant".into();
        assert!(app.visible().is_empty());
        app.navigate(true);
    }
    #[test]
    fn native_ui_lays_out_all_states_and_details_at_multiple_sizes() {
        for width in [440.0, 800.0, 1440.0] {
            let ctx = egui::Context::default();
            let mut app = StudioApp::test_app(&ctx);
            for selected in [None, Some(0), Some(7)] {
                app.selected = selected;
                for _ in 0..3 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 900.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| app.show(ctx),
                    );
                    assert!(!output.shapes.is_empty());
                }
            }
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ctx| app.show(ctx),
            );
            assert_eq!(app.selected, None);
        }
    }
}
