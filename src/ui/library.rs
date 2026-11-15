use super::*;

pub(super) fn columns(width: f32) -> usize {
    ((width / 270.0).floor() as usize).clamp(1, 5)
}
#[derive(Clone, Copy)]
pub(super) enum Action {
    Open,
    Run,
    OfficialInstaller,
    Details,
    Pause,
    Cancel,
}
impl StudioApp {
    pub(super) fn persist_preferences(&mut self) {
        self.preferences.sort = self.sort;
        if let Some(path) = &self.preferences_path {
            if let Err(e) = self.preferences.save(path) {
                self.notice = Some(format!("Enregistrement des préférences impossible : {e}"));
            }
        }
    }
    pub(super) fn apply_appearance(&self, ctx: &egui::Context) {
        let dark = match self.preferences.appearance {
            Appearance::System => {
                ctx.system_theme().unwrap_or(egui::Theme::Dark) == egui::Theme::Dark
            }
            Appearance::Dark => true,
            Appearance::Light => false,
        };
        if ctx.style().visuals.dark_mode != dark {
            theme::apply_mode(ctx, dark);
        }
    }
    pub(super) fn library(&mut self, ctx: &egui::Context) {
        let (bg, border, muted, blue) = theme::palette(ctx);
        egui::TopBottomPanel::top("brand")
            .frame(
                egui::Frame::new()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(24, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (mark, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
                    // Studio App: three distinct modules, consistent with assets/brand/studio-app.svg.
                    for (x, y, w, h, color) in [
                        (0.0, 0.0, 22.0, 5.0, Color32::from_rgb(162, 104, 232)),
                        (0.0, 8.0, 9.0, 14.0, Color32::from_rgb(52, 144, 242)),
                        (12.0, 8.0, 10.0, 14.0, Color32::from_rgb(127, 95, 229)),
                    ] {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(mark.min + egui::vec2(x, y), egui::vec2(w, h)),
                            2.0,
                            color,
                        );
                    }
                    ui.label(RichText::new("STUDIO APP").strong().size(22.0));
                    if ui.available_width() > 340.0 {
                        ui.add_space(18.0);
                        ui.label(mono("L’ATELIER DES CRÉATEURS").color(muted));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_sized([36.0, 34.0], egui::Button::new("⚙"))
                            .on_hover_text("Réglages")
                            .clicked()
                        {
                            self.settings_open = true;
                            self.selected = None;
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(bg)
                    .stroke(Stroke::new(1.0_f32, border))
                    .inner_margin(egui::Margin::symmetric(24, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        mono(
                            if self.apps.iter().any(|app| app.manual_release.is_some()) {
                                "INSTALLATEUR OFFICIEL · OUVERTURE NATIVE"
                            } else if self.apps.iter().any(|app| !app.fictional) {
                                "INSTALLATION ET OUVERTURE NATIVES"
                            } else {
                                "DÉMO FICTIVE · OPÉRATIONS SIMULÉES"
                            },
                        )
                        .color(blue),
                    );
                    ui.separator();
                    if self.job.is_some() {
                        ui.spinner();
                    }
                    let status = if self.native_status.is_empty() {
                        self.catalog_status.clone()
                    } else {
                        format!("{} · {}", self.catalog_status, self.native_status)
                    };
                    ui.add(egui::Label::new(mono(&status).color(muted)).truncate())
                        .on_hover_text(status);
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(24))
            .show(ctx, |ui| {
                self.toolbar(ui);
                ui.add_space(20.0);
                if let Some(notice) = self.notice.clone() {
                    egui::Frame::new()
                        .stroke(Stroke::new(1.0_f32, border))
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(blue, notice);
                                if ui.button("FERMER").clicked() {
                                    self.notice = None;
                                }
                            });
                        });
                    ui.add_space(10.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt("library_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let indices = self.visible();
                        if indices.is_empty() {
                            let has_visible_apps = self.apps.iter().any(|a| !a.fictional || self.preferences.show_demo_apps);
                            ui.heading(if self.apps.is_empty() {
                                "Catalogue indisponible"
                            } else if !has_visible_apps {
                                "Aucune application réelle disponible"
                            } else {
                                "Aucune application trouvée"
                            });
                            if self.apps.is_empty() {
                                ui.label("Actualisez le catalogue ou vérifiez sa configuration.");
                            } else if !has_visible_apps {
                                ui.label("Les démonstrations sont masquées dans les réglages.");
                            } else if ui.button("EFFACER LA RECHERCHE").clicked() {
                                self.query.clear();
                            }
                        }
                        for installed in [true, false] {
                            let group: Vec<_> = indices
                                .iter()
                                .copied()
                                .filter(|&i| self.apps[i].installed() == installed)
                                .collect();
                            if group.is_empty() {
                                continue;
                            }
                            ui.label(
                                mono(format!(
                                    "{} ({})",
                                    if installed {
                                        if group
                                            .iter()
                                            .all(|&i| self.apps[i].simulation.version.is_some())
                                        {
                                            "VOTRE ATELIER · ÉTATS SIMULÉS"
                                        } else {
                                            "APPLICATIONS INSTALLÉES"
                                        }
                                    } else {
                                        "APPLICATIONS DISPONIBLES"
                                    },
                                    group.len()
                                ))
                                .color(muted),
                            );
                            ui.add_space(14.0);
                            if self.preferences.view == View::Grid {
                                let count = columns(ui.available_width());
                                let width = ui.available_width() / count as f32;
                                for row in group.chunks(count) {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.0;
                                        for &i in row {
                                            self.card(ui, i, width);
                                        }
                                    });
                                    ui.add_space(-10.0);
                                }
                            } else {
                                for i in group {
                                    self.list_row(ui, i);
                                    ui.add_space(-10.0);
                                }
                            }
                            ui.add_space(30.0);
                        }
                    });
            });
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let old = (self.sort, self.preferences.view);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 38.0;
            if ui.available_width() >= 1050.0 {
                ui.horizontal(|ui| {
                    self.refresh_button(ui);
                    let count = self.update_count();
                    let tail = if count > 0 { 838.0 } else { 600.0 };
                    ui.add_space((ui.available_width() - tail).max(0.0));
                    self.update_all(ui);
                    self.sort_selector(ui);
                    self.search(ui, 240.0);
                    self.view_toggle(ui);
                });
            } else {
                ui.horizontal(|ui| {
                    self.refresh_button(ui);
                    self.update_all(ui);
                });
                ui.horizontal(|ui| {
                    self.sort_selector(ui);
                    self.view_toggle(ui);
                });
                self.search(ui, ui.available_width());
            }
        });
        if old != (self.sort, self.preferences.view) {
            self.persist_preferences();
        }
    }
    fn refresh_button(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                self.job.is_none(),
                egui::Button::new("⟳").min_size(egui::vec2(36.0, 38.0)),
            )
            .on_hover_text("Actualiser le catalogue")
            .clicked()
        {
            self.refresh_catalog(ui.ctx());
        }
    }
    fn update_count(&self) -> usize {
        self.apps
            .iter()
            .filter(|a| self.preferences.show_demo_apps && a.fictional && a.state() == AppState::UpdateAvailable)
            .count()
    }
    fn update_all(&mut self, ui: &mut egui::Ui) {
        let count = self.update_count();
        if count > 0
            && primary(ui, &format!("TOUT METTRE À JOUR ({count})"), 230.0)
                .on_hover_text("Simuler les mises à jour disponibles")
                .clicked()
        {
            for app in &mut self.apps {
                if app.fictional && app.state() == AppState::UpdateAvailable {
                    demo::start(app);
                }
            }
        }
    }
    fn sort_selector(&mut self, ui: &mut egui::Ui) {
        ui.label(mono("TRI"));
        egui::ComboBox::from_id_salt("sort")
            .width(145.0)
            .selected_text(match self.sort {
                Sort::Name => "Nom (A–Z)",
                Sort::Category => "Catégorie",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.sort, Sort::Name, "Nom (A–Z)");
                ui.selectable_value(&mut self.sort, Sort::Category, "Catégorie");
            });
    }
    fn search(&mut self, ui: &mut egui::Ui, width: f32) {
        ui.add_sized(
            [width, 38.0],
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Rechercher une application…")
                .margin(egui::vec2(10.0, 8.0)),
        );
    }
    fn view_toggle(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (view, label, tip) in [
                (View::Grid, "▦", "Afficher la grille"),
                (View::List, "☰", "Afficher la liste"),
            ] {
                if ui
                    .add_sized(
                        [38.0, 38.0],
                        egui::Button::new(label).selected(self.preferences.view == view),
                    )
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.preferences.view = view;
                }
            }
        });
    }
    pub(super) fn dispatch(&mut self, ctx: &egui::Context, index: usize, action: Option<Action>) {
        match action {
            Some(Action::Details) => self.select(index),
            Some(Action::Run) => self.action(index),
            Some(Action::OfficialInstaller) => {
                if let Some(page) = self.apps[index].manual_installer_page() {
                    ctx.open_url(egui::OpenUrl::new_tab(page));
                }
            }
            Some(Action::Open) => self.open_application(index),
            Some(Action::Pause) => {
                if let Operation::SimulatedDownload { paused, .. } = &mut self.apps[index].operation
                {
                    *paused = !*paused;
                }
            }
            Some(Action::Cancel) => {
                if self.apps[index].fictional {
                    demo::cancel(&mut self.apps[index]);
                } else if let Some(job) = &self.installer_job {
                    if job.application.id == self.apps[index].id {
                        job.cancel.cancel();
                    }
                }
            }
            None => {}
        }
    }
    fn card(&mut self, ui: &mut egui::Ui, index: usize, width: f32) {
        let app = &self.apps[index];
        let (_, border, muted, blue) = theme::palette(ui.ctx());
        let color = theme::hover::accent(ui.ctx(), app.color);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 306.0), Sense::hover());
        let response = ui.interact(rect, ui.id().with((&app.id, "card")), Sense::click());
        let installed = app.installed();
        let hovered = ui.rect_contains_pointer(rect);
        let reduced = self.motion.reduced();
        let progress = theme::hover::progress(ui.ctx(), &app.id, "grid", hovered, reduced);
        let fill = theme::hover::fill(ui.ctx(), app.color, installed, progress);
        ui.painter().rect_filled(rect, 0, fill);
        ui.painter().rect_stroke(
            rect,
            0,
            Stroke::new(1.0_f32, border),
            egui::StrokeKind::Inside,
        );
        theme::hover::line(ui.painter(), rect, color, progress);
        let mut action = None;
        let mut strip = ui.new_child(
            egui::UiBuilder::new()
                .id_salt((&app.id, "strip"))
                .max_rect(egui::Rect::from_min_max(
                    rect.min + egui::vec2(16.0, 8.0),
                    egui::pos2(rect.right() - 16.0, rect.top() + 30.0),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        strip
            .add_sized(
                [(width - 130.0).max(80.0), 22.0],
                egui::Label::new(mono(version(app)).color(if installed {
                    color
                } else {
                    theme::hover::mix(muted, color, progress)
                }))
                .truncate(),
            )
            .on_hover_text(version(app));
        strip.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(link) = &app.links.github {
                ui.hyperlink_to(mono("GITHUB ↗"), link);
            } else {
                ui.label(mono(if app.fictional { "FICTIF" } else { "CATALOGUE" }).size(10.0));
            }
        });
        ui.painter().hline(
            rect.x_range(),
            rect.top() + 34.0,
            Stroke::new(1.0_f32, border),
        );
        let mut body = ui.new_child(
            egui::UiBuilder::new()
                .layout(Layout::top_down(Align::Min))
                .id_salt((&app.id, "body"))
                .max_rect(egui::Rect::from_min_max(
                    rect.min + egui::vec2(16.0, 49.0),
                    rect.max - egui::vec2(16.0, 58.0),
                )),
        );
        body.spacing_mut().item_spacing.x = 10.0;
        body.horizontal(|ui| {
            let (slot, _) = ui.allocate_exact_size(egui::vec2(54.0, 54.0), Sense::hover());
            paint_logo(
                ui.painter(),
                theme::hover::logo_rect(slot, progress, reduced),
                app,
                &self.images,
                &self.gray_logos,
                if installed { 1.0 } else { progress },
            );
            ui.vertical(|ui| {
                if ui
                    .add(
                        egui::Label::new(theme::title(&app.name, 22.0))
                            .truncate()
                            .sense(Sense::click()),
                    )
                    .on_hover_text(&app.name)
                    .clicked()
                {
                    action = Some(Action::Details);
                }
                ui.add(
                    egui::Label::new(RichText::new(&app.category).size(12.0).color(muted))
                        .truncate(),
                )
                .on_hover_text(&app.category);
            });
        });
        body.add_space(9.0);
        let status = app.state();
        let status_color = if status == AppState::UpdateAvailable {
            if ui.visuals().dark_mode {
                Color32::from_rgb(255, 199, 74)
            } else {
                Color32::from_rgb(126, 85, 0)
            }
        } else {
            blue
        };
        body.horizontal(|ui| {
            egui::Frame::new()
                .stroke(Stroke::new(1.0_f32, border))
                .inner_margin(egui::Margin::symmetric(5, 3))
                .show(ui, |ui| {
                    ui.label(mono(status_label(app)).color(status_color).size(10.0));
                });
        });
        body.add_space(8.0);
        body.label(
            RichText::new(if app.manual_release.is_some() {
                if app.manual_installer_page().is_some() {
                    "Installateur officiel sur GitHub"
                } else {
                    "Plateforme non prise en charge"
                }
                .into()
            } else if app.release.packages.is_empty() {
                if app.fictional {
                    "Aucun package approuvé · simulation"
                } else {
                    "Aucun package approuvé"
                }
                .into()
            } else {
                format!(
                    "Téléchargement : {:.1} MB{}",
                    app.size_mb(),
                    if app.fictional { " · simulation" } else { "" }
                )
            })
            .size(12.0)
            .color(muted),
        );
        let (description, _) =
            body.allocate_exact_size(egui::vec2(body.available_width(), 36.0), Sense::hover());
        let mut job = egui::text::LayoutJob::simple(
            app.short_description.clone(),
            egui::FontId::proportional(13.0),
            muted,
            description.width(),
        );
        job.wrap.max_rows = 2;
        let galley = body.painter().layout_job(job);
        body.painter().galley(description.min, galley, muted);
        body.add(
            egui::Label::new(
                mono(format!("{} · {}", app.publisher, app.release.published))
                    .size(10.0)
                    .color(muted),
            )
            .truncate(),
        );
        let mut buttons = ui.new_child(
            egui::UiBuilder::new()
                .id_salt((&app.id, "buttons"))
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 16.0, rect.bottom() - 54.0),
                    egui::vec2(width - 32.0, 38.0),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        actions(&mut buttons, app, &mut action);
        if response.clicked() && action.is_none() {
            action = Some(Action::Details);
        }
        self.dispatch(ui.ctx(), index, action);
    }
    fn list_row(&mut self, ui: &mut egui::Ui, index: usize) {
        let app = &self.apps[index];
        let (_, border, muted, _) = theme::palette(ui.ctx());
        let wide = ui.available_width() > 880.0;
        let mut action = None;
        let background = ui.painter().add(egui::Shape::Noop);
        let mut logo_slot = None;
        let response = egui::Frame::new()
            .stroke(Stroke::new(1.0_f32, border))
            .inner_margin(16)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    logo_slot = Some(
                        ui.allocate_exact_size(egui::vec2(42.0, 42.0), Sense::hover())
                            .0,
                    );
                    ui.vertical(|ui| {
                        if wide {
                            ui.set_width(170.0);
                        }
                        if ui
                            .add(
                                egui::Label::new(theme::title(&app.name, 20.0))
                                    .truncate()
                                    .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            action = Some(Action::Details);
                        }
                        ui.label(mono(version(app)).color(if app.installed() {
                            theme::hover::accent(ui.ctx(), app.color)
                        } else {
                            muted
                        }));
                    });
                    if wide {
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 370.0).max(90.0));
                            ui.add(egui::Label::new(&app.short_description).truncate())
                                .on_hover_text(&app.short_description);
                            ui.add(
                                egui::Label::new(mono(status_label(app)).color(muted)).truncate(),
                            );
                        });
                        ui.allocate_ui_with_layout(
                            egui::vec2(330.0, 38.0),
                            Layout::left_to_right(Align::Center),
                            |ui| actions(ui, app, &mut action),
                        );
                    }
                });
                if !wide {
                    ui.label(mono(status_label(app)));
                    ui.add(egui::Label::new(&app.short_description).truncate())
                        .on_hover_text(&app.short_description);
                    ui.horizontal(|ui| actions(ui, app, &mut action));
                }
            })
            .response;
        let hovered = ui.rect_contains_pointer(response.rect);
        let reduced = self.motion.reduced();
        let progress = theme::hover::progress(ui.ctx(), &app.id, "list", hovered, reduced);
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect,
                0,
                theme::hover::fill(ui.ctx(), app.color, app.installed(), progress),
            ),
        );
        theme::hover::line(
            ui.painter(),
            response.rect,
            theme::hover::accent(ui.ctx(), app.color),
            progress,
        );
        if let Some(slot) = logo_slot {
            paint_logo(
                ui.painter(),
                theme::hover::logo_rect(slot, progress, reduced),
                app,
                &self.images,
                &self.gray_logos,
                if app.installed() { 1.0 } else { progress },
            );
        }
        response.on_hover_text(if app.fictional {
            "Opérations simulées. Cliquez sur le nom ou sur … pour ouvrir la fiche."
        } else {
            "Cliquez sur le nom ou sur … pour ouvrir la fiche."
        });
        self.dispatch(ui.ctx(), index, action);
    }
}
fn status_label(app: &Application) -> String {
    let state = app.state();
    let label = if app.manual_version_is_ambiguous() {
        "INSTALLÉ · BÊTA NON IDENTIFIÉE"
    } else if !app.fictional && !app.local.installations.is_empty() && app.local.version.is_none() {
        "INSTALLÉ · VERSION INCONNUE"
    } else if !app.fictional
        && app.local.version.as_ref().is_some_and(|v| {
            crate::native::model::compare_versions(v, app.available_version()).is_none()
        })
    {
        "INSTALLÉ · VERSION NON COMPARABLE"
    } else {
        state.label()
    };
    if app.fictional && (app.simulation.version.is_some() || app.operation != Operation::Idle) {
        format!("{label} · SIMULÉ")
    } else {
        label.into()
    }
}
fn version(app: &Application) -> String {
    match app.displayed_installed_version() {
        Some(v)
            if !app.manual_version_is_ambiguous()
                && crate::native::model::compare_versions(v, app.available_version())
                    == Some(std::cmp::Ordering::Less) =>
        {
            format!("{v} → {}", app.available_version())
        }
        Some(v) => format!("V{v}"),
        None if app.installed() => "VERSION LOCALE INCONNUE".into(),
        None => format!("V{}", app.available_version()),
    }
}

pub(super) fn actions(ui: &mut egui::Ui, app: &Application, action: &mut Option<Action>) {
    ui.spacing_mut().item_spacing.x = 6.0;
    let width = (ui.available_width() - 44.0).max(110.0);
    if let Operation::Native(progress) = &app.operation {
        ui.vertical(|ui| {
            ui.set_width((width - 32.0).max(80.0));
            if progress.phase == crate::installer::Phase::Downloading {
                ui.add(
                    egui::Label::new(
                        mono(format!(
                            "{:.1}/{:.1} MB · {:.0}%",
                            progress.bytes as f64 / 1e6,
                            progress.total as f64 / 1e6,
                            progress.fraction() * 100.0
                        ))
                        .size(10.0),
                    )
                    .truncate(),
                )
                .on_hover_text(
                    progress
                        .bytes_per_second
                        .map(|n| format!("Débit moyen : {:.1} MB/s", n as f64 / 1e6))
                        .unwrap_or_else(|| "Débit en cours de mesure".into()),
                );
                ui.add(
                    egui::ProgressBar::new(progress.fraction())
                        .fill(accent(app.color))
                        .desired_height(4.0),
                );
            } else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.add(egui::Label::new(mono(progress.phase.label()).size(9.0)).truncate());
                });
            }
        });
        if ui
            .add_enabled(progress.cancellable, egui::Button::new("×"))
            .on_hover_text(if progress.cancellable {
                "Annuler l’opération réelle"
            } else {
                "La copie finale ne peut pas être interrompue"
            })
            .clicked()
        {
            *action = Some(Action::Cancel);
        }
        return;
    }
    match app.state() {
        AppState::Downloading { progress, paused } => {
            ui.vertical(|ui| {
                ui.set_width((width - 76.0).max(60.0));
                ui.label(mono(format!("SIMULÉ · {:.0}%", progress * 100.0)));
                ui.add(
                    egui::ProgressBar::new(progress)
                        .fill(accent(app.color))
                        .desired_height(4.0),
                );
            });
            if ui
                .button(if paused { "▶" } else { "Ⅱ" })
                .on_hover_text("Suspendre / reprendre la simulation")
                .clicked()
            {
                *action = Some(Action::Pause);
            }
            if ui
                .button("×")
                .on_hover_text("Annuler la simulation")
                .clicked()
            {
                *action = Some(Action::Cancel);
            }
        }
        AppState::Installing { progress } => {
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.label(mono("INSTALLATION FICTIVE"));
                ui.add(
                    egui::ProgressBar::new(progress)
                        .fill(accent(app.color))
                        .desired_height(4.0),
                );
            });
        }
        state => {
            if app.installed() {
                let update = state == AppState::UpdateAvailable;
                let manual = app.manual_installer_page().is_some();
            let ambiguous = manual && app.manual_version_is_ambiguous();
            let open_width = if update || ambiguous { width * 0.36 } else { width };
                if ui
                    .add_sized(
                        [open_width, 38.0],
                        egui::Button::new(mono("OUVRIR").strong()),
                    )
                    .on_hover_text(if app.fictional {
                        "Simuler l’ouverture : aucun programme lancé"
                    } else {
                        "Ouvrir l’installation locale détectée"
                    })
                    .clicked()
                {
                    *action = Some(Action::Open);
                }
                if (update || ambiguous)
                    && ui
                        .add_enabled_ui(app.fictional || manual, |ui| {
                            primary(ui, if ambiguous { "VERSIONS ↗" } else if manual { "MISE À JOUR ↗" } else { "METTRE À JOUR" }, width - open_width - 6.0)
                        })
                        .inner
                        .on_hover_text(if app.fictional {
                            "Simuler la mise à jour"
                        } else if ambiguous {
                            "Le bundle local ne déclare pas sa révision bêta ; consulter la release officielle"
                        } else if manual {
                            "Ouvrir la release officielle ; quitter l’application avant la mise à jour manuelle"
                        } else {
                            "Mise à jour non prise en charge à cette étape"
                        })
                        .clicked()
                {
                    *action = Some(if manual {Action::OfficialInstaller} else {Action::Run});
                }
            } else if app.manual_installer_page().is_some() {
                if primary(ui,"INSTALLATEUR ↗",width).on_hover_text("Ouvrir la release officielle et suivre le guide d’installation").clicked() {
                    *action=Some(Action::OfficialInstaller);
                }
            } else if ui
                .add_enabled_ui(app.fictional || (crate::installer::model::can_request(&app.metadata) && !matches!(&app.operation,Operation::NativeError(e) if !e.retryable)), |ui| {
                    primary(
                        ui,
                        if matches!(state, AppState::Error { .. }) {
                            "RÉESSAYER"
                        } else {
                            "INSTALLER"
                        },
                        width,
                    )
                })
                .inner
                .on_hover_text(if app.fictional {
                    "Simuler l’installation : aucun logiciel téléchargé"
                } else {
                    "Installer le package approuvé : authenticité et intégrité seront vérifiées avant installation"
                })
                .clicked()
            {
                *action = Some(Action::Run);
            }
        }
    }
    ui.menu_button("…", |ui| {
        if ui.button("Voir la fiche").clicked() {
            *action = Some(Action::Details);
            ui.close();
        }
        if ui
            .add_enabled(
                app.fictional,
                egui::Button::new("Réinitialiser la simulation"),
            )
            .clicked()
        {
            *action = Some(Action::Cancel);
            ui.close();
        }
        ui.add_enabled(false, egui::Button::new("Désinstaller"))
            .on_hover_text("Disponible dans une prochaine version.");
    });
}
