//! Native GPU capture harness. Does not emulate or rasterize the UI off-screen.
use eframe::egui;
struct Capture {
    app: studio_app::ui::StudioApp,
    frame: u32,
    requested: bool,
    path: std::path::PathBuf,
    video: Option<std::path::PathBuf>,
    start: Option<std::time::Instant>,
    next_sample: u32,
    samples: Vec<(u32, f64, std::sync::Arc<egui::ColorImage>)>,
}
impl eframe::App for Capture {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        if self.video.is_some() {
            let time = self.start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
            input.events.push(egui::Event::PointerMoved(
                if (0.5..1.3).contains(&time) || (2.0..2.8).contains(&time) {
                    egui::pos2(160.0, 350.0)
                } else {
                    egui::pos2(10.0, 70.0)
                },
            ));
        } else if self.frame >= 50
            && std::env::var("STUDIO_QA_STATE").is_ok_and(|s| s.contains("hover"))
        {
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
            if let egui::Event::Screenshot {
                image, user_data, ..
            } = event
            {
                if self.video.is_some() {
                    if let Some(&(index, time)) = user_data
                        .data
                        .as_ref()
                        .and_then(|d| d.downcast_ref::<(u32, f64)>())
                    {
                        self.samples.push((index, time, image));
                    }
                    continue;
                }
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
        if let Some(directory) = &self.video {
            if self.frame >= 50 {
                let elapsed = self
                    .start
                    .get_or_insert_with(std::time::Instant::now)
                    .elapsed()
                    .as_secs_f64();
                if elapsed >= 3.3 && !self.requested {
                    self.requested = true;
                    std::fs::create_dir_all(directory).expect("video directory");
                    let mut concat = String::new();
                    for (i, (index, time, image)) in self.samples.iter().enumerate() {
                        let name = format!("frame-{index:04}.png");
                        let bytes: Vec<_> =
                            image.pixels.iter().flat_map(|p| p.to_array()).collect();
                        image::save_buffer(
                            directory.join(&name),
                            &bytes,
                            image.size[0] as u32,
                            image.size[1] as u32,
                            image::ColorType::Rgba8,
                        )
                        .expect("video frame");
                        let duration = self
                            .samples
                            .get(i + 1)
                            .map(|next| next.1 - time)
                            .unwrap_or(1.0 / 30.0);
                        concat.push_str(&format!("file '{name}'\nduration {duration:.6}\n"));
                    }
                    std::fs::write(directory.join("frames.txt"), concat).expect("video timeline");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else if elapsed < 3.3 && elapsed >= self.next_sample as f64 / 30.0 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                        (self.next_sample, elapsed),
                    )));
                    self.next_sample += 1;
                }
            }
        } else if self.frame >= 80 && !self.requested {
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
                video: std::env::var_os("STUDIO_QA_VIDEO").map(std::path::PathBuf::from),
                start: None,
                next_sample: 0,
                samples: Vec::new(),
            }))
        }),
    )
}
