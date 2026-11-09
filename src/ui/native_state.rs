use super::*;
impl StudioApp {
    pub(super) fn poll_native(&mut self, ctx: &egui::Context) {
        match receive(&mut self.native_job) {
            Ok(Some(snapshot)) if snapshot.generation == self.native_generation => {
                self.apply_native(snapshot)
            }
            Err(()) => {
                self.native_status = "Détection locale interrompue. Actualisez le catalogue.".into()
            }
            _ => {}
        }
        if self.native_pending && self.native_job.is_none() {
            self.native_pending = false;
            let requests = self
                .apps
                .iter()
                .filter(|a| !a.fictional)
                .filter_map(|a| {
                    a.detection
                        .clone()
                        .map(|rules| crate::native::worker::Request {
                            id: a.id.clone(),
                            rules,
                        })
                })
                .collect::<Vec<_>>();
            if requests.is_empty() {
                self.native_status.clear();
            } else {
                self.native_status = "Détection locale…".into();
                let ctx = ctx.clone();
                self.native_job = Some(crate::native::worker::spawn(
                    self.native_generation,
                    requests,
                    move || ctx.request_repaint(),
                ));
            }
        }
        match receive(&mut self.launch_job) {
            Ok(Some((id, result))) => {
                self.notice = Some(match result {
                    Ok(()) => format!("Demande d’ouverture native transmise pour {id}."),
                    Err(e) => format!("{id} : {e}"),
                });
                self.native_pending = true;
            }
            Err(()) => {
                self.notice = Some("Le worker de lancement s’est arrêté sans résultat.".into());
                self.native_pending = true;
            }
            _ => {}
        }
        if self.native_pending || self.native_job.is_some() || self.launch_job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    fn apply_native(&mut self, snapshot: crate::native::worker::Snapshot) {
        let mut count = 0;
        for (request, report) in snapshot.entries {
            if let Some(app) = self.apps.iter_mut().find(|a| {
                a.id == request.id && !a.fictional && a.detection.as_ref() == Some(&request.rules)
            }) {
                app.local.version = report.installations.first().and_then(|i| i.version.clone());
                app.local.installations = report.installations;
                app.local.errors = report.errors;
                count += usize::from(!app.local.installations.is_empty());
            }
        }
        self.native_status = format!("Détection locale : {count} application(s)");
    }
    pub(super) fn open_application(&mut self, index: usize) {
        let app = &self.apps[index];
        if app.fictional {
            self.notice = Some(format!(
                "Ouverture simulée de {}. Aucun programme n’a été lancé.",
                app.name
            ));
            return;
        }
        if self.launch_job.is_some() {
            self.notice = Some("Une demande d’ouverture est déjà en cours.".into());
            return;
        }
        let Some((rules, installation)) = app
            .detection
            .clone()
            .zip(app.local.installations.first().cloned())
        else {
            self.notice =
                Some("Aucune installation locale vérifiée. Actualisez le catalogue.".into());
            return;
        };
        let id = app.id.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.launch_job = Some(rx);
        std::thread::spawn(move || {
            let result = crate::native::launch(&rules, &installation);
            let _ = tx.send((id, result));
        });
    }
}

fn receive<T>(job: &mut Option<Receiver<T>>) -> Result<Option<T>, ()> {
    let Some(receiver) = job.as_ref() else {
        return Ok(None);
    };
    match receiver.try_recv() {
        Ok(value) => {
            *job = None;
            Ok(Some(value))
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            *job = None;
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn real_app(ctx: &egui::Context) -> StudioApp {
        let mut app = StudioApp::test_app(ctx);
        app.apps.truncate(1);
        app.apps[0].metadata.fictional = false;
        app.apps[0].metadata.detection = Some(crate::native::DetectionRules {
            macos: Some(crate::native::model::MacRule {
                bundle_id: "org.studio.fixture".into(),
                bundle_paths: vec!["Fixture.app".into()],
            }),
            windows: None,
        });
        app.apps[0].simulation = Default::default();
        app
    }
    #[test]
    fn disconnected_worker_is_reported_without_busy_loop() {
        let ctx = egui::Context::default();
        let mut app = real_app(&ctx);
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        app.native_job = Some(rx);
        app.poll_native(&ctx);
        assert!(app.native_job.is_none());
        assert!(app.native_status.contains("interrompue"));
    }
    #[test]
    fn native_results_never_add_an_application_outside_the_catalogue() {
        let ctx = egui::Context::default();
        let mut app = real_app(&ctx);
        let ids = app.apps.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        let request = crate::native::worker::Request {
            id: "unreferenced-third-party".into(),
            rules: app.apps[0].detection.clone().unwrap(),
        };
        app.apply_native(crate::native::worker::Snapshot {
            generation: 0,
            entries: vec![(request, Default::default())],
        });
        assert_eq!(
            app.apps.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
            ids
        );
        assert!(app.apps[0].local.installations.is_empty());
        let default = crate::catalog::parse(include_bytes!("../../catalog/catalog.json")).unwrap();
        assert!(default.applications.iter().all(|a| a.id != "obs-studio"));
    }
    #[test]
    fn snapshots_match_generation_ids_and_rules_and_unknown_versions_are_installed() {
        let ctx = egui::Context::default();
        let mut app = real_app(&ctx);
        let request = crate::native::worker::Request {
            id: app.apps[0].id.clone(),
            rules: app.apps[0].detection.clone().unwrap(),
        };
        let installation = crate::native::Installation {
            path: "/fixture/Fixture.app".into(),
            executable: "/fixture/Fixture.app/Contents/MacOS/Fixture".into(),
            version: None,
            source: crate::native::Source::MacBundle,
        };
        app.apply_native(crate::native::worker::Snapshot {
            generation: 0,
            entries: vec![(
                request.clone(),
                crate::native::Report {
                    installations: vec![installation.clone()],
                    errors: vec![],
                },
            )],
        });
        assert!(app.apps[0].installed());
        assert_eq!(app.apps[0].state(), AppState::Installed);
        assert!(app.apps[0].simulation.version.is_none());
        let (tx, rx) = std::sync::mpsc::channel();
        app.native_job = Some(rx);
        app.native_generation = 2;
        tx.send(crate::native::worker::Snapshot {
            generation: 1,
            entries: vec![(request, crate::native::Report::default())],
        })
        .unwrap();
        app.poll_native(&ctx);
        assert!(app.apps[0].installed(), "stale snapshot discarded");
        let mut metadata = app.apps[0].metadata.clone();
        metadata
            .detection
            .as_mut()
            .unwrap()
            .macos
            .as_mut()
            .unwrap()
            .bundle_id = "org.studio.changed".into();
        app.replace_catalog(vec![metadata], false);
        assert!(!app.apps[0].installed());
    }
    #[test]
    fn real_app_never_enters_simulator_and_open_failure_is_reported() {
        let ctx = egui::Context::default();
        let mut app = real_app(&ctx);
        demo::start(&mut app.apps[0]);
        assert_eq!(app.apps[0].operation, Operation::Idle);
        app.open_application(0);
        assert!(app.notice.as_ref().unwrap().contains("Aucune installation"));
        assert!(app.launch_job.is_none());
        app.apps[0]
            .local
            .installations
            .push(crate::native::Installation {
                path: "/missing/Fixture.app".into(),
                executable: "/missing/Fixture.app/Contents/MacOS/Test".into(),
                version: None,
                source: crate::native::Source::MacBundle,
            });
        app.open_application(0);
        let result = app
            .launch_job
            .take()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(result.1.is_err());
        assert!(app.apps[0].simulation.version.is_none());
    }
    #[test]
    fn real_versions_equal_older_newer_and_unknown_preserve_available_metadata() {
        let ctx = egui::Context::default();
        let mut app = real_app(&ctx);
        let available = app.apps[0].available_version().to_owned();
        for (version, state) in [
            ("1.8.0", AppState::Installed),
            ("1.6.0", AppState::UpdateAvailable),
            ("2.0.0", AppState::Installed),
            ("vendor build", AppState::Installed),
        ] {
            app.apps[0].local.version = Some(version.into());
            assert_eq!(app.apps[0].state(), state);
            assert_eq!(app.apps[0].available_version(), available);
        }
    }
}
