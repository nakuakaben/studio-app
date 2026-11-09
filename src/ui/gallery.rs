//! Original, code-drawn concept previews; no third-party screenshots or assets.
use super::theme::{self, BORDER, MUTED};
use crate::domain::Application;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};

pub const CAPTIONS: [&str; 3] = [
    "L’ESPACE DE CRÉATION",
    "LES OUTILS DE PRÉCISION",
    "LA COMPOSITION FINALE",
];

pub fn paint_image(p: &egui::Painter, rect: Rect, texture: &egui::TextureHandle) {
    let size = texture.size_vec2();
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let fitted = Rect::from_center_size(rect.center(), size * scale);
    p.image(
        texture.id(),
        fitted,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

pub fn preview(
    ui: &mut egui::Ui,
    app: &Application,
    slide: usize,
    width: f32,
    images: &std::collections::HashMap<String, egui::TextureHandle>,
) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(egui::vec2(width, width * 0.59), Sense::click());
    let p = ui.painter_at(r);
    if let Some(screenshot) = app.screenshots.get(slide) {
        p.rect_filled(r, 0, theme::BG);
        if let Some(texture) = images.get(&screenshot.image.source) {
            paint_image(&p, r, texture);
        } else {
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                "Capture indisponible",
                FontId::proportional(12.0),
                MUTED,
            );
        }
        return response.on_hover_text(&screenshot.image.alt);
    }
    if !app.fictional {
        p.rect_filled(r, 0, theme::BG);
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            "Aucune capture fournie",
            FontId::proportional(12.0),
            MUTED,
        );
        return response;
    }
    let color = theme::accent(app.color);
    p.rect_filled(r, 0, Color32::from_rgb(25, 27, 32));
    let at = |x: f32, y: f32| Pos2::new(r.left() + x * r.width(), r.top() + y * r.height());
    let box_at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_max(at(x, y), at(x + w, y + h));
    p.rect_filled(
        box_at(0.0, 0.0, 1.0, 0.07),
        0,
        Color32::from_rgb(35, 37, 43),
    );
    if width > 250.0 {
        p.text(
            at(0.025, 0.035),
            Align2::LEFT_CENTER,
            format!("{}  /  ATELIER CONCEPTUEL", app.name.to_uppercase()),
            FontId::monospace(9.0),
            MUTED,
        );
    }
    for i in 0..7 {
        let y = 0.11 + i as f32 * 0.055;
        p.rect_filled(box_at(0.025, y, 0.14, 0.018), 0, BORDER);
    }
    let canvas = box_at(0.21, 0.1, 0.59, 0.56);
    p.rect_filled(canvas, 0, Color32::from_rgb(11, 13, 19));
    for i in 0..8 {
        let x = 0.22 + i as f32 * 0.08;
        p.line_segment(
            [at(x, 0.11), at(x, 0.65)],
            Stroke::new(0.5_f32, Color32::from_gray(33)),
        );
    }
    let center = canvas.center();
    let radius = canvas.height() * 0.32;
    match (slide + app.id.len()) % 3 {
        0 => {
            for i in 0..4 {
                let offset = i as f32 * radius * 0.21;
                p.circle_stroke(
                    center + egui::vec2(offset - radius * 0.3, 0.0),
                    radius - offset * 0.2,
                    Stroke::new(2.0_f32, color.gamma_multiply(1.0 - i as f32 * 0.16)),
                );
            }
            p.line_segment(
                [canvas.left_bottom(), canvas.right_top()],
                Stroke::new(1.0_f32, color.gamma_multiply(0.4)),
            );
        }
        1 => {
            let points: Vec<_> = (0..6)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 6.0;
                    center + egui::vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            p.add(egui::Shape::convex_polygon(
                points,
                color.gamma_multiply(0.23),
                Stroke::new(2.0_f32, color),
            ));
            p.line_segment(
                [
                    center - egui::vec2(radius, 0.0),
                    center + egui::vec2(radius, 0.0),
                ],
                Stroke::new(1.0_f32, color),
            );
            p.circle_filled(center, radius * 0.3, color);
        }
        _ => {
            for i in 0..5 {
                let offset = i as f32 * radius * 0.25;
                let sq = Rect::from_center_size(
                    center + egui::vec2(offset - radius * 0.5, -offset + radius * 0.5),
                    egui::vec2(radius * 1.3, radius * 1.3),
                );
                p.rect_stroke(
                    sq,
                    0,
                    Stroke::new(2.0_f32, color.gamma_multiply(0.35 + i as f32 * 0.15)),
                    StrokeKind::Inside,
                );
            }
        }
    }
    for i in 0..6 {
        let y = 0.14 + i as f32 * 0.07;
        p.rect_filled(box_at(0.84, y, 0.12, 0.012), 0, BORDER);
        p.rect_filled(
            box_at(0.84, y + 0.025, 0.04 + i as f32 * 0.01, 0.012),
            0,
            color.gamma_multiply(0.7),
        );
    }
    for track in 0..4 {
        for clip in 0..5 {
            let x = 0.21 + clip as f32 * 0.145 + if track % 2 == 0 { 0.0 } else { 0.04 };
            let y = 0.72 + track as f32 * 0.055;
            p.rect_filled(
                box_at(x, y, 0.13, 0.038),
                1,
                color.gamma_multiply(0.3 + track as f32 * 0.12),
            );
        }
    }
    p.line_segment(
        [at(0.51, 0.69), at(0.51, 0.96)],
        Stroke::new(1.0_f32, Color32::WHITE),
    );
    p.rect_stroke(r, 0, Stroke::new(1.0_f32, BORDER), StrokeKind::Inside);
    response.on_hover_text("Aperçu conceptuel original · logiciel fictif")
}
