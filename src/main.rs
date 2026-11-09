#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Studio App — Bibliothèque",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 960.0])
                .with_min_inner_size([440.0, 560.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(studio_app::ui::StudioApp::new(&cc.egui_ctx)))),
    )
}
