#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn inspect(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    use studio_app::catalog::{
        github,
        loader::{self, HttpsTransport, Settings},
    };
    match args {
        [flag] | [flag, _] if flag == "--check-catalog" => {
            let (default_config, cache) = loader::default_paths()?;
            let config = args
                .get(1)
                .map(std::path::Path::new)
                .unwrap_or(&default_config);
            let settings = Settings::from_path(config, &cache)?;
            let loaded = loader::load(&settings, &HttpsTransport)?;
            println!(
                "{:?} · {} · {} application(s)",
                loaded.origin,
                loaded.catalog.name,
                loaded.catalog.applications.len()
            );
            if let Some(warning) = loaded.warning {
                eprintln!("{warning}");
            }
            Ok(true)
        }
        [flag, repo] if flag == "--discover" => {
            let (owner, repository) = repo
                .split_once('/')
                .ok_or("Format attendu : owner/repository")?;
            let source = studio_app::catalog::model::GitHubSource {
                owner: owner.into(),
                repository: repository.into(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&github::discover(&source, &HttpsTransport)?)?
            );
            Ok(true)
        }
        [] => Ok(false),
        _ => Err(
            "Usage : studio-app [--check-catalog [config.json] | --discover owner/repository]"
                .into(),
        ),
    }
}
fn main() -> eframe::Result<()> {
    match inspect(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(true) => return Ok(()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
        Ok(false) => {}
    }
    eframe::run_native(
        "Studio App — Bibliothèque",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 960.0])
                .with_icon(std::sync::Arc::new(studio_app::brand::icon_data()))
                .with_min_inner_size([440.0, 560.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(studio_app::ui::StudioApp::new(&cc.egui_ctx)))),
    )
}
