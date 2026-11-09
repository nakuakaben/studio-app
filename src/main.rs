fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Studio App",
        options,
        Box::new(|_cc| Ok(Box::new(StudioApp))),
    )
}

struct StudioApp;

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Studio App");
            ui.label("Socle technique — catalogue et cartes à développer.");
        });
    }
}
