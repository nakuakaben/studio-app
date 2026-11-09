//! Read-only diagnostic, independent of the GUI. Never launches any software.
use std::{fs::File, io::Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: cargo run --example detect -- catalogue.json")?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(studio_app::catalog::MAX_CATALOG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let catalog = studio_app::catalog::parse(&bytes)?;
    for app in catalog.applications {
        if !app.fictional {
            if let Some(rules) = app.detection {
                let report = studio_app::native::detect(&rules);
                println!("{}: {} installation(s)", app.id, report.installations.len());
                for installed in report.installations {
                    println!(
                        "  {} — {}",
                        installed.path.display(),
                        installed.version.as_deref().unwrap_or("version inconnue")
                    );
                }
                for error in report.errors {
                    eprintln!("  {error}");
                }
            }
        }
    }
    Ok(())
}
