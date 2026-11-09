//! Minimal test software. It writes only inside its marked, isolated installation.
fn main() {
    if std::env::args().any(|a| a == "--version") {
        println!("0.0.1");
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(root) = executable.ancestors().find(|p| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("StudioAppInstallerTest-"))
    }) else {
        return;
    };
    let Ok(token) = std::fs::read_to_string(root.join(".studio-app-installer-fixture")) else {
        return;
    };
    if token.len() != 16
        || !token.bytes().all(|b| b.is_ascii_hexdigit())
        || root.file_name().unwrap().to_string_lossy() != format!("StudioAppInstallerTest-{token}")
    {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("probe-launched.txt"))
    {
        use std::io::Write;
        let _ = file.write_all(b"Studio App Installer Test 0.0.1 launched\n");
    }
}
