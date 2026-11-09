//! Local development server; no production signing key or package publication.
use studio_app::{
    catalog::model::PackageFormat,
    installer::{fixture::Fixture, Failure},
};
fn main() -> Result<(), Failure> {
    let mut args = std::env::args_os().skip(1);
    let probe = args.next().ok_or_else(|| {
        Failure::permanent("Usage: studio-installer-fixture <probe executable> [zip|dmg]")
    })?;
    if probe == "--cleanup" {
        let path = args
            .next()
            .ok_or_else(|| Failure::permanent("--cleanup <installer-policy.json du test>"))?;
        return studio_app::installer::fixture::cleanup_policy(std::path::Path::new(&path));
    }
    let format = match args
        .next()
        .as_deref()
        .and_then(|s| s.to_str())
        .unwrap_or("zip")
    {
        "zip" => PackageFormat::Zip,
        "dmg" => PackageFormat::Dmg,
        _ => return Err(Failure::permanent("Format de test: zip ou dmg")),
    };
    let fixture = Fixture::new(std::path::Path::new(&probe), format, true)?;
    println!("STUDIO_APP_CONFIG={}\nSTUDIO_APP_INSTALLER_POLICY={}\nSTUDIO_APP_PREFERENCES_DIR={}\nSTUDIO_APP_CACHE_DIR={}\nDESTINATION={}\nAppuyez sur Entrée pour arrêter le serveur et nettoyer uniquement ce test.",fixture.workspace.path().join("config.json").display(),fixture.workspace.path().join("installer-policy.json").display(),fixture.workspace.path().join("preferences").display(),fixture.workspace.path().join("cache").display(),fixture.root.display());
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    fixture.cleanup()?;
    Ok(())
}
