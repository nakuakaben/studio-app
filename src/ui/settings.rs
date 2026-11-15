use super::*;
const FUTURE: &str = "Disponible dans une prochaine version.";
impl StudioApp {
    pub(super) fn settings(&mut self, ctx: &egui::Context) {
        let (bg, border, muted, _) = theme::palette(ctx);
        let size = ctx.content_rect().size();
        let mut done = false;
        let before = (self.preferences.appearance, self.preferences.language);
        let modal = egui::Modal::new(egui::Id::new("launcher_settings"))
            .frame(
                egui::Frame::new()
                    .fill(bg)
                    .stroke(Stroke::new(1.0_f32, border)),
            )
            .show(ctx, |ui| {
                ui.set_width((size.x - 48.0).clamp(300.0, 1080.0));
                ui.spacing_mut().item_spacing.y = 6.0;
                let (line, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), Sense::hover());
                ui.painter()
                    .rect_filled(line, 0, Color32::from_rgb(160, 100, 230));
                egui::Frame::new()
                    .fill(theme::tinted(ctx, [160, 100, 230]))
                    .inner_margin(20)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.heading(theme::title("Réglages", 30.0));
                        ui.label(
                            mono(format!(
                                "STUDIO APP LAUNCHER · {}",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .color(muted),
                        );
                    });
                egui::ScrollArea::vertical()
                    .id_salt("settings_scroll")
                    .max_height((size.y - 260.0).clamp(180.0, 620.0))
                    .show(ui, |ui| {
                        egui::Frame::new().inner_margin(22).show(ui, |ui| {
                            if ui.available_width() > 820.0 {
                                ui.spacing_mut().item_spacing.x = 24.0;
                                ui.columns(2, |columns| {
                                    self.settings_left(&mut columns[0]);
                                    self.settings_right(&mut columns[1]);
                                });
                            } else {
                                self.settings_left(ui);
                                self.settings_right(ui);
                            }
                        });
                    });
                ui.separator();
                egui::Frame::new().inner_margin(18).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("QUITTER STUDIO APP").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            done = ui
                                .add_enabled_ui(self.preferences.validate().is_ok(), |ui| {
                                    primary(ui, "TERMINÉ", 110.0)
                                })
                                .inner
                                .clicked();
                        });
                    });
                });
            });
        if before != (self.preferences.appearance, self.preferences.language) {
            self.persist_preferences();
            self.apply_appearance(ctx);
        }
        if done || modal.should_close() {
            if self.preferences.validate().is_err() {
                let previous = self
                    .preferences_path
                    .as_deref()
                    .and_then(|p| Preferences::load(p).ok())
                    .unwrap_or_default();
                self.preferences.community_website = previous.community_website;
                self.preferences.community_discord = previous.community_discord;
            }
            self.persist_preferences();
            self.settings_open = false;
        }
    }
    fn settings_left(&mut self, ui: &mut egui::Ui) {
        section(ui, "LANGUE", |ui| {
            egui::ComboBox::from_id_salt("language")
                .selected_text(match self.preferences.language {
                    Language::Auto => "Automatique",
                    Language::French => "Français",
                })
                .width(ui.available_width() - 20.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.preferences.language,
                        Language::Auto,
                        "Automatique",
                    );
                    ui.selectable_value(
                        &mut self.preferences.language,
                        Language::French,
                        "Français",
                    );
                });
            ui.small("Français disponible. Automatique utilise le français par défaut.");
        });
        section(ui, "BIBLIOTHÈQUE", |ui| {
            if ui.checkbox(
                &mut self.preferences.show_demo_apps,
                "Afficher les applications de démonstration",
            ).changed() {
                self.selected = None;
                self.persist_preferences();
            }
            ui.small("Désactivé par défaut. Les applications fictives restent disponibles pour les essais.");
        });
        section(ui, "MISES À JOUR", |ui| {
            ui.add_enabled_ui(false, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Rechercher des mises à jour");
                    egui::ComboBox::from_id_salt("update_frequency")
                        .selected_text("Manuellement")
                        .show_ui(ui, |_| {});
                });
                for label in [
                    "Télécharger et installer automatiquement",
                    "Me prévenir des mises à jour",
                    "Inclure les versions bêta",
                ] {
                    ui.checkbox(&mut false, label);
                }
            });
            ui.small(FUTURE);
            ui.add_space(6.0);
            ui.label(format!(
                "Version du launcher : {}",
                env!("CARGO_PKG_VERSION")
            ));
            ui.small("Version locale — aucune mise à jour automatique.");
        });
        section(ui, "APPARENCE", |ui| {
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [
                    (Appearance::System, "COMME LE SYSTÈME"),
                    (Appearance::Dark, "SOMBRE"),
                    (Appearance::Light, "CLAIR"),
                ] {
                    ui.selectable_value(&mut self.preferences.appearance, value, mono(label));
                }
            });
        });
    }
    fn settings_right(&mut self, ui: &mut egui::Ui) {
        section(ui, "DOSSIER D’INSTALLATION PAR DÉFAUT", |ui| {
            ui.label("Dossier d’installation prévu");
            let mut path = if cfg!(target_os = "macos") {
                String::from("/Applications/Studio App")
            } else if cfg!(target_os = "windows") {
                String::from(r"%LOCALAPPDATA%\Studio App\Applications")
            } else {
                String::from("~/.local/share/studio-app/applications")
            };
            ui.add_enabled(
                false,
                egui::TextEdit::singleline(&mut path).desired_width(f32::INFINITY),
            );
            ui.add_enabled_ui(false, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let _ = ui.button("PARCOURIR…");
                    let _ = ui.button("PAR DÉFAUT");
                });
            });
            ui.small(FUTURE);
            ui.small("Aucun dossier d’installation n’est créé à cette étape.");
        });
        section(ui, "EXÉCUTION EN ARRIÈRE-PLAN", |ui| {
            ui.add_enabled_ui(false, |ui| {
                for label in [
                    "Rester actif dans la zone de notification",
                    "Afficher la fenêtre sur le bureau courant",
                    "Lancer Studio App au démarrage de session",
                ] {
                    ui.checkbox(&mut false, label);
                }
            });
            ui.small(FUTURE);
        });
        section(ui, "COMMUNAUTÉ", |ui| {
            ui.label("Liens officiels configurables (HTTPS)");
            let mut website = self
                .preferences
                .community_website
                .clone()
                .unwrap_or_default();
            let mut discord = self
                .preferences
                .community_discord
                .clone()
                .unwrap_or_default();
            ui.label("Site web");
            let site_changed = ui
                .add(
                    egui::TextEdit::singleline(&mut website)
                        .hint_text("Non configuré")
                        .desired_width(f32::INFINITY),
                )
                .lost_focus();
            ui.label("Discord");
            let discord_changed = ui
                .add(
                    egui::TextEdit::singleline(&mut discord)
                        .hint_text("Non configuré")
                        .desired_width(f32::INFINITY),
                )
                .lost_focus();
            // Keep editing buffers across frames; validate before any clickable link or disk write.
            self.preferences.community_website =
                (!website.trim().is_empty()).then(|| website.trim().to_owned());
            self.preferences.community_discord =
                (!discord.trim().is_empty()).then(|| discord.trim().to_owned());
            if let Err(error) = self.preferences.validate() {
                ui.colored_label(Color32::LIGHT_RED, error);
            } else {
                ui.horizontal_wrapped(|ui| {
                    if let Some(url) = &self.preferences.community_website {
                        ui.hyperlink_to("SITE WEB ↗", url);
                    }
                    if let Some(url) = &self.preferences.community_discord {
                        ui.hyperlink_to("DISCORD ↗", url);
                    }
                    ui.hyperlink_to("GITHUB ↗", "https://github.com/nakuakaben/studio-app");
                });
                if site_changed || discord_changed {
                    self.persist_preferences();
                }
            }
        });
    }
}
fn section(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let (_, border, muted, _) = theme::palette(ui.ctx());
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, border))
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(mono(title).color(muted));
            ui.add_space(8.0);
            contents(ui);
        });
    ui.add_space(12.0);
}
