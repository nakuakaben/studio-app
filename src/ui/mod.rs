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
use theme::{accent, mono, BG, BLUE, BORDER, MUTED, WHITE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Name,
    Category,
}

pub struct StudioApp {
    apps: Vec<Application>,
    query: String,
    sort: Sort,
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
    pub fn new(ctx: &egui::Context) -> Self {
        let mut app = Self::empty(ctx);
        app.refresh_catalog(ctx);
        app
    }

    fn empty(ctx: &egui::Context) -> Self {
        theme::apply(ctx);
        Self {
            apps: Vec::new(),
            query: String::new(),
            sort: Sort::Name,
            selected: None,
            slide: 0,
            notice: None,
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
        if self.apps[index].state() == AppState::Installed {
            self.notice = Some(format!(
                "Ouverture simulée de {}. Aucun programme n’a été lancé.",
                self.apps[index].name
            ));
        } else {
            demo::start(&mut self.apps[index]);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.poll_catalog(ctx);
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
        egui::TopBottomPanel::top("brand")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(24, 18)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let (mark, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
                    for (x, y) in [(0.0, 0.0), (12.0, 0.0), (0.0, 12.0)] {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                mark.min + egui::vec2(x, y),
                                egui::vec2(9.0, 9.0),
                            ),
                            0,
                            WHITE,
                        );
                    }
                    ui.label(RichText::new("STUDIO APP").strong().size(23.0));
                    ui.add_space(22.0);
                    ui.label(mono("L’ATELIER DES CRÉATEURS"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(mono("V0.1 / SIMULATION").color(BLUE));
                    });
                });
            });
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(mono("CATALOGUE DYNAMIQUE · OPÉRATIONS SIMULÉES"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(mono("RUST / NATIF"));
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(24))
            .show(ctx, |ui| {
                self.toolbar(ui);
                ui.add_space(16.0);
                ui.horizontal_wrapped(|ui| {
                    if self.job.is_some() {
                        ui.spinner();
                    }
                    ui.label(mono(&self.catalog_status));
                    if ui
                        .add_enabled(
                            self.job.is_none(),
                            egui::Button::new("ACTUALISER LE CATALOGUE"),
                        )
                        .clicked()
                    {
                        self.refresh_catalog(ctx);
                    }
                });
                ui.add_space(10.0);
                if let Some(notice) = self.notice.clone() {
                    theme::outline().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(notice).color(BLUE));
                            if ui.button("FERMER").clicked() {
                                self.notice = None;
                            }
                        });
                    });
                    ui.add_space(8.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt("library_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let indices = self.visible();
                        if indices.is_empty() && !self.apps.is_empty() {
                            ui.add_space(80.0);
                            ui.heading("Aucune application trouvée");
                            ui.label(
                                RichText::new("Essayez un autre nom ou une catégorie.")
                                    .color(MUTED),
                            );
                            if ui.button("EFFACER LA RECHERCHE").clicked() {
                                self.query.clear();
                            }
                        }
                        for installed in [true, false] {
                            let group: Vec<_> = indices
                                .iter()
                                .copied()
                                .filter(|&i| {
                                    self.apps[i].displayed_installed_version().is_some()
                                        == installed
                                })
                                .collect();
                            if group.is_empty() {
                                continue;
                            }
                            ui.add_space(10.0);
                            ui.label(mono(if installed {
                                "VOTRE ATELIER / ÉTATS SIMULÉS"
                            } else {
                                "À DÉCOUVRIR"
                            }));
                            ui.add_space(10.0);
                            let columns = ((ui.available_width() / 320.0).floor() as usize).max(1);
                            let width = ui.available_width() / columns as f32;
                            for row in group.chunks(columns) {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    for &index in row {
                                        self.card(ui, index, width);
                                    }
                                });
                                ui.add_space(-10.0);
                            }
                            ui.add_space(30.0);
                        }
                    });
            });
        if let Some(index) = self.selected {
            self.details(ctx, index);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        if ui.available_width() >= 1000.0 {
            ui.horizontal(|ui| {
                ui.label(mono("BIBLIOTHÈQUE"));
                ui.label(mono(format!("/ {:02} APPLICATIONS", self.apps.len())));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.controls(ui));
            });
        } else {
            ui.label(mono(format!(
                "BIBLIOTHÈQUE / {:02} APPLICATIONS",
                self.apps.len()
            )));
            ui.horizontal_wrapped(|ui| self.controls(ui));
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Rechercher une application…")
                .desired_width(240.0_f32.min(ui.available_width().max(120.0)))
                .margin(egui::vec2(12.0, 10.0)),
        );
        egui::ComboBox::from_id_salt("sort")
            .selected_text(match self.sort {
                Sort::Name => "Nom (A–Z)",
                Sort::Category => "Catégorie",
            })
            .width(140.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.sort, Sort::Name, "Nom (A–Z)");
                ui.selectable_value(&mut self.sort, Sort::Category, "Catégorie");
            });
        let count = self
            .apps
            .iter()
            .filter(|a| a.state() == AppState::UpdateAvailable)
            .count();
        if ui
            .add_enabled(
                count > 0,
                egui::Button::new(mono(format!("SIMULER LES MAJ ({count})")).color(WHITE)),
            )
            .clicked()
        {
            for app in &mut self.apps {
                if app.state() == AppState::UpdateAvailable {
                    demo::start(app);
                }
            }
        }
    }

    fn card(&mut self, ui: &mut egui::Ui, index: usize, width: f32) {
        let mut open = false;
        let mut action = false;
        let mut toggle_pause = false;
        let mut cancel = false;
        let app = &self.apps[index];
        let color = accent(app.color);
        let frame = egui::Frame::new()
            .fill(if app.state() == AppState::UpdateAvailable {
                theme::tint(app.color)
            } else {
                BG
            })
            .stroke(Stroke::new(1.0_f32, BORDER));
        let card_response = ui.interact(
            egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(width, 360.0)),
            ui.id().with(&app.id).with("card"),
            Sense::click(),
        );
        ui.allocate_ui_with_layout(
            egui::vec2(width, 360.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.x = 10.0;
                frame
                    .show(ui, |ui| {
                        ui.set_width(width - 2.0);
                        ui.set_height(358.0);
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(18, 11))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let version = match app.displayed_installed_version() {
                                        Some(v) if v != app.available_version() => {
                                            format!("{v} → {}", app.available_version())
                                        }
                                        _ => format!("V{}", app.available_version()),
                                    };
                                    ui.label(mono(version).color(color));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        open |= ui.small_button(mono("FICHE ↗")).clicked();
                                    });
                                });
                            });
                        ui.separator();
                        egui::Frame::new().inner_margin(18).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                logo(ui, app, 54.0, &self.images);
                                ui.vertical(|ui| {
                                    ui.add_space(1.0);
                                    open |= ui
                                        .add(
                                            egui::Label::new(
                                                RichText::new(&app.name).size(25.0).strong(),
                                            )
                                            .sense(Sense::click())
                                            .truncate(),
                                        )
                                        .clicked();
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&app.category).color(MUTED).size(13.0),
                                        )
                                        .truncate(),
                                    );
                                });
                            });
                            ui.add_space(17.0);
                            let status_color = match app.state() {
                                AppState::UpdateAvailable => Color32::from_rgb(255, 199, 74),
                                AppState::Error { .. } => Color32::from_rgb(255, 132, 116),
                                _ => BLUE,
                            };
                            egui::Frame::new()
                                .fill(BG)
                                .stroke(Stroke::new(1.0_f32, BORDER))
                                .inner_margin(egui::Margin::symmetric(6, 3))
                                .show(ui, |ui| {
                                    ui.label(
                                        mono(app.state().label()).size(10.0).color(status_color),
                                    );
                                });
                            ui.add_space(13.0);
                            let (description_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 56.0),
                                Sense::hover(),
                            );
                            let mut job = egui::text::LayoutJob::simple(
                                app.short_description.clone(),
                                egui::FontId::proportional(14.0),
                                MUTED,
                                description_rect.width(),
                            );
                            job.wrap.max_rows = 3;
                            let galley = ui.painter().layout_job(job);
                            ui.painter().galley(description_rect.min, galley, MUTED);
                            ui.add(
                                egui::Label::new(
                                    mono(format!("{} · {}", app.publisher, app.release.published))
                                        .size(10.0),
                                )
                                .truncate(),
                            );
                            ui.add_space(12.0);
                            match &app.state() {
                                AppState::Downloading { progress, paused } => {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width((width - 126.0).max(80.0));
                                            ui.label(
                                                mono(format!("SIMULÉ · {:.0}%", *progress * 100.0))
                                                    .size(10.0),
                                            );
                                            ui.add(
                                                egui::ProgressBar::new(*progress)
                                                    .fill(color)
                                                    .corner_radius(0)
                                                    .desired_height(4.0),
                                            );
                                        });
                                        toggle_pause = ui
                                            .button(if *paused { "▶" } else { "Ⅱ" })
                                            .on_hover_text("Reprendre / suspendre la simulation")
                                            .clicked();
                                        cancel = ui
                                            .button("×")
                                            .on_hover_text("Annuler la simulation")
                                            .clicked();
                                    });
                                }
                                AppState::Installing { progress } => {
                                    ui.label(
                                        mono(format!(
                                            "INSTALLATION FICTIVE · {:.0}%",
                                            *progress * 100.0
                                        ))
                                        .color(color),
                                    );
                                    ui.add(
                                        egui::ProgressBar::new(*progress)
                                            .fill(color)
                                            .corner_radius(0)
                                            .desired_height(4.0),
                                    );
                                }
                                _ => {
                                    action =
                                        primary(ui, app.state().action(), ui.available_width())
                                            .clicked();
                                }
                            }
                        });
                    })
                    .response
            },
        );
        if card_response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            && !action
            && !toggle_pause
            && !cancel
        {
            open = true;
        }
        if open {
            self.select(index);
        }
        if action {
            self.action(index);
        }
        if toggle_pause {
            if let Operation::SimulatedDownload { paused, .. } = &mut self.apps[index].operation {
                *paused = !*paused;
            }
        }
        if cancel {
            demo::cancel(&mut self.apps[index]);
        }
    }

    fn details(&mut self, ctx: &egui::Context, index: usize) {
        let mut close = false;
        let mut previous = false;
        let mut next = false;
        let mut action = false;
        let app = &self.apps[index];
        let color = accent(app.color);
        let size = ctx.content_rect().size();
        let modal = egui::Modal::new(egui::Id::new("application_details")).frame(egui::Frame::new().fill(BG).stroke(Stroke::new(1.0_f32,BORDER))).show(ctx, |ui| {
            ui.set_width((size.x - 64.0).clamp(320.0, 1140.0));
            ui.set_height(size.y - 100.0);
            let (line,_) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), Sense::hover());
            ui.painter().rect_filled(line, 0, color);
            egui::ScrollArea::vertical().id_salt(("detail_scroll", &app.id)).max_height(size.y - 100.0).show(ui, |ui| {
                egui::Frame::new().fill(theme::tint(app.color)).inner_margin(24).show(ui, |ui| {
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
                            ui.heading(RichText::new(&app.name).size(34.0).strong());
                            ui.label(&app.category);
                            if let Some(notice) = &self.notice { ui.colored_label(BLUE, notice); }
                            ui.label(RichText::new(match app.displayed_installed_version() { Some(v) => format!("{v} simulée / {} disponible", app.available_version()), None => format!("Version disponible {}", app.available_version()) }).color(MUTED).size(13.0));
                        });
                    });
                    ui.add_space(16.0);
                    ui.horizontal_wrapped(|ui| {
                        action = ui.add_enabled_ui(!app.state().busy(), |ui| primary(ui, app.state().action(), 260.0)).inner.clicked();
                        for (label, link) in [("SITE WEB ↗", &app.links.website), ("GITHUB ↗", &app.links.github), ("DOCUMENTATION ↗", &app.links.documentation)] {
                            if let Some(link) = link { ui.hyperlink_to(label, link); }
                        }
                    });
                    ui.label(mono("SIMULATION LOCALE · AUCUN LOGICIEL N’EST TÉLÉCHARGÉ OU EXÉCUTÉ").size(10.0));
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
        egui::Button::new(mono(text).strong().color(BG)).fill(WHITE),
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
        theme::outline().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(mono(format!("{:02}", i + 1)).color(accent(app.color)));
                ui.add_space(10.0);
                ui.strong(&feature.title);
            });
            ui.label(RichText::new(&feature.description).color(MUTED).size(14.0));
        });
    }
}

fn release(ui: &mut egui::Ui, app: &Application) {
    ui.label(mono("02 / VERSION DU CATALOGUE"));
    ui.add_space(10.0);
    theme::outline().show(ui, |ui| {
        ui.label(mono("VERSION DISPONIBLE"));
        ui.label(app.available_version());
        ui.label(mono("VERSION LOCALE DÉTECTÉE"));
        ui.label(app.local.version.as_deref().unwrap_or("Non vérifiée"));
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
        ui.label(RichText::new(format!("· {note}")).color(MUTED).size(14.0));
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
        for _ in 0..2 {
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

    #[test]
    fn actual_pointer_clicks_trigger_actions_and_open_details() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        click_text(&mut app, &ctx, "SIMULER LA MISE À JOUR");
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
