use super::{
    assets,
    loader::{self, HttpsTransport, Loaded, Origin, Settings},
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};
pub enum Event {
    Catalog {
        loaded: Loaded,
        demo: bool,
    },
    Asset {
        source: String,
        result: super::Result<assets::Raster>,
    },
    Failed(String),
    Done,
}
pub fn spawn(
    config: PathBuf,
    cache: PathBuf,
    repaint: impl Fn() + Send + 'static,
) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let send = |event| {
            let ok = tx.send(event).is_ok();
            repaint();
            ok
        };
        let result = Settings::from_path(&config, &cache).and_then(|settings| {
            let loaded = loader::load(&settings, &HttpsTransport)?;
            let sources: Vec<_> = assets::references(&loaded.catalog)
                .iter()
                .map(|a| a.source.clone())
                .collect();
            let offline = loaded.origin == Origin::Cache;
            if !offline && loaded.warning.is_none() {
                let _ = settings.cache.retain_images(&sources);
            }
            if !send(Event::Catalog {
                loaded,
                demo: settings.config.demo_scenarios,
            }) {
                return Ok(());
            }
            let mut total = 0;
            let started = std::time::Instant::now();
            for source in sources {
                let load = if offline || started.elapsed().as_secs() >= 30 {
                    assets::load_cached(&settings, &source)
                } else {
                    assets::load(&settings, &source, &HttpsTransport)
                };
                let result = load.and_then(|raster| {
                    if total + raster.rgba.len() > assets::MAX_TOTAL_RGBA {
                        return Err(super::Error::invalid(
                            "Budget cumulé d’images dépassé (64 MiB)",
                        ));
                    }
                    total += raster.rgba.len();
                    Ok(raster)
                });
                if !send(Event::Asset { source, result }) {
                    return Ok(());
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            send(Event::Failed(error.to_string()));
        }
        send(Event::Done);
    });
    rx
}
