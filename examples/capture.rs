//! Native GPU capture harness. Does not emulate or rasterize the UI off-screen.
use eframe::egui;
struct Capture {
    app: studio_app::ui::StudioApp,
    frame: u32,
    requested: bool,
    path: std::path::PathBuf,
}
impl eframe::App for Capture {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        if self.frame >= 50 && std::env::var("STUDIO_QA_STATE").is_ok_and(|s| s.contains("hover")) {
            input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(160.0, 580.0)));
        }
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        #[cfg(feature = "visual-qa")]
        if self.frame == 35 {
            self.app
                .prepare_visual_qa(&std::env::var("STUDIO_QA_STATE").unwrap_or_default());
        }
        self.app.show(ctx);
        self.frame += 1;
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                image::save_buffer(
                    &self.path,
                    &bytes,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .expect("write native screenshot");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.frame >= 80 && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(20));
    }
}
fn main() -> eframe::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .expect("capture output path");
    let width = std::env::var("STUDIO_QA_WIDTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1664.0);
    let height = std::env::var("STUDIO_QA_HEIGHT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(940.0);
    eframe::run_native(
        "Studio App · contrôle visuel",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([width, height]),
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Capture {
                app: studio_app::ui::StudioApp::new(&cc.egui_ctx),
                frame: 0,
                requested: false,
                path,
            }))
        }),
    )
}
