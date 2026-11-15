use super::*;
use crate::catalog::{loader, worker};
impl StudioApp {
    pub(super) fn refresh_catalog(&mut self, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        match loader::default_paths() {
            Ok((config, cache)) => {
                self.catalog_status = "Chargement du catalogue…".into();
                let ctx = ctx.clone();
                self.job = Some(worker::spawn(config, cache, move || ctx.request_repaint()));
            }
            Err(error) => self.catalog_status = format!("Catalogue indisponible : {error}"),
        }
    }
    pub(super) fn replace_catalog(
        &mut self,
        metadata: Vec<crate::catalog::model::AppMetadata>,
        seed: bool,
    ) {
        let selected_id = self
            .selected
            .and_then(|i| self.apps.get(i))
            .map(|a| a.id.clone());
        let mut old: HashMap<_, _> = std::mem::take(&mut self.apps)
            .into_iter()
            .map(|a| (a.id.clone(), a))
            .collect();
        let first = old.is_empty();
        self.apps = metadata
            .into_iter()
            .map(|metadata| {
                if let Some(mut app) = old.remove(&metadata.id) {
                    // A deliberate refresh allows a corrected local approval to be checked again.
                    if matches!(app.operation,Operation::NativeError(_)){app.operation=Operation::Idle;}
                    if app.metadata.detection != metadata.detection
                        || app.metadata.fictional != metadata.fictional
                    {
                        app.local = Default::default();
                    }
                    if app.metadata.fictional != metadata.fictional {
                        app.simulation = Default::default();
                        app.operation = Operation::Idle;
                    }
                    if matches!(app.operation,Operation::Native(_)) && serde_json::to_vec(&app.metadata).ok()!=serde_json::to_vec(&metadata).ok() {
                        if let Some(job)=&self.installer_job {job.cancel.cancel();}
                        app.operation=Operation::NativeError(crate::installer::Failure::permanent("Catalogue modifié pendant l’opération ; annulation demandée. Actualisez après la fin de l’opération pour vérifier l’état local."));
                    }
                    app.metadata = metadata;
                    app.manual_release = None;
                    app
                } else {
                    Application::new(metadata)
                }
            })
            .collect();
        if seed && first {
            demo::seed(&mut self.apps);
        }
        self.selected = selected_id.and_then(|id| self.apps.iter().position(|a| a.id == id));
        self.slide = 0;
        self.images.clear();
        self.gray_logos.clear();
        self.asset_errors.clear();
        self.native_generation = self.native_generation.wrapping_add(1);
        self.native_pending = true;
    }
    pub(super) fn poll_catalog(&mut self, ctx: &egui::Context) {
        let events: Vec<_> = self
            .job
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Catalog { loaded, demo } => {
                    self.catalog_status = format!(
                        "{} · {}{}",
                        loaded.catalog.name,
                        match loaded.origin {
                            Origin::Local => "fichier local",
                            Origin::Remote => "HTTPS",
                            Origin::Cache => "données en cache",
                        },
                        loaded
                            .warning
                            .as_ref()
                            .map(|w| format!(" · {w}"))
                            .unwrap_or_default()
                    );
                    self.catalog_origin = Some(loaded.origin);
                    self.replace_catalog(loaded.catalog.applications, demo);
                    if let Some(handoff) = loaded.manual_release {
                        if let Some(app) = self
                            .apps
                            .iter_mut()
                            .find(|app| app.id == handoff.application_id)
                        {
                            app.manual_release = Some(handoff);
                        }
                    }
                }
                Event::Asset { source, result } => match result {
                    Ok(raster) => {
                        let image =
                            egui::ColorImage::from_rgba_unmultiplied(raster.size, &raster.rgba);
                        if self
                            .apps
                            .iter()
                            .any(|a| a.logo.as_ref().is_some_and(|logo| logo.source == source))
                        {
                            let mut gray = image.clone();
                            for pixel in &mut gray.pixels {
                                *pixel = theme::hover::gray(*pixel);
                            }
                            self.gray_logos.insert(
                                source.clone(),
                                ctx.load_texture(
                                    "catalog-logo-gray",
                                    gray,
                                    egui::TextureOptions::LINEAR,
                                ),
                            );
                        }
                        self.images.insert(
                            source,
                            ctx.load_texture("catalog-image", image, egui::TextureOptions::LINEAR),
                        );
                    }
                    Err(error) => {
                        self.asset_errors.insert(source, error.to_string());
                    }
                },
                Event::Failed(error) => {
                    self.catalog_status = format!(
                        "Catalogue indisponible : {error}{}",
                        if self.apps.is_empty() {
                            ""
                        } else {
                            " · dernière vue conservée"
                        }
                    )
                }
                Event::Done => self.job = None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_catalog_and_images_render_in_existing_cards_and_details() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::empty(&ctx);
        let (sender, receiver) = std::sync::mpsc::channel();
        app.job = Some(receiver);
        let loaded = loader::Loaded {
            catalog: crate::catalog::parse(include_bytes!("../../tests/fixtures/valid.json"))
                .unwrap(),
            manual_release: None,
            origin: Origin::Cache,
            warning: Some("Hors ligne".into()),
        };
        sender
            .send(Event::Catalog {
                loaded,
                demo: false,
            })
            .unwrap();
        sender
            .send(Event::Asset {
                source: "sample.png".into(),
                result: crate::catalog::assets::decode(include_bytes!(
                    "../../tests/fixtures/sample.png"
                )),
            })
            .unwrap();
        sender.send(Event::Done).unwrap();
        app.poll_catalog(&ctx);
        assert!(app.catalog_status.contains("données en cache"));
        assert_eq!(app.images.len(), 1);
        assert_eq!(app.gray_logos.len(), 1);
        assert!(app.apps[0].local.version.is_none());
        assert!(app.apps[0].simulation.version.is_none());
        for width in [440.0, 800.0, 1440.0] {
            for selected in [None, Some(0)] {
                app.selected = selected;
                for _ in 0..3 {
                    let out = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 900.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| app.show(ctx),
                    );
                    assert!(!out.shapes.is_empty());
                }
            }
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        app.job = Some(receiver);
        sender
            .send(Event::Failed("Réseau indisponible".into()))
            .unwrap();
        sender.send(Event::Done).unwrap();
        app.poll_catalog(&ctx);
        assert_eq!(app.apps.len(), 1);
        assert!(app.catalog_status.contains("dernière vue conservée"));
    }
    #[test]
    fn refresh_reconciles_by_id_and_closes_removed_detail_without_installation_claims() {
        let ctx = egui::Context::default();
        let mut app = StudioApp::test_app(&ctx);
        app.select(0);
        let local = app.apps[0].local.clone();
        let mut catalog =
            crate::catalog::parse(include_bytes!("../../catalog/catalog.json")).unwrap();
        catalog.applications.reverse();
        catalog
            .applications
            .iter_mut()
            .find(|a| a.id == "sequence")
            .unwrap()
            .release
            .version = "2.0.0".into();
        app.replace_catalog(catalog.applications.clone(), false);
        assert_eq!(app.apps[app.selected.unwrap()].id, "sequence");
        assert_eq!(app.apps[app.selected.unwrap()].local, local);
        assert_eq!(
            app.apps[app.selected.unwrap()]
                .simulation
                .version
                .as_deref(),
            Some("1.6.0")
        );
        catalog.applications.retain(|a| a.id != "sequence");
        let mut added = catalog.applications[0].clone();
        added.id = "new-app".into();
        catalog.applications.push(added);
        app.replace_catalog(catalog.applications, false);
        assert_eq!(app.selected, None);
        assert_eq!(app.apps.last().unwrap().id, "new-app");
        assert!(app.apps.iter().all(|a| a.local.version.is_none()));
    }
}
