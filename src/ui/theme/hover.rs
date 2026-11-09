//! Library-only paint transitions. Layout rectangles never depend on animation.
use eframe::egui::{self, Color32, Id, Rect, Stroke};

pub const DURATION: f32 = 0.22;
pub const LOGO_SCALE: f32 = 1.04;

pub fn progress(
    ctx: &egui::Context,
    app_id: &str,
    view: &str,
    hovered: bool,
    reduced: bool,
) -> f32 {
    let id = Id::new(("library-hover", app_id, view));
    let t = ctx.animate_bool_with_time(id, hovered, if reduced { 0.0 } else { DURATION });
    // Smoothstep has no overshoot and is symmetric on pointer exit.
    t * t * (3.0 - 2.0 * t)
}

pub fn logo_rect(slot: Rect, progress: f32, reduced: bool) -> Rect {
    Rect::from_center_size(
        slot.center(),
        slot.size()
            * if reduced {
                1.0
            } else {
                1.0 + (LOGO_SCALE - 1.0) * progress
            },
    )
}

pub fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let a = a.to_array();
    let b = b.to_array();
    let channel = |i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * amount).round() as u8;
    Color32::from_rgba_premultiplied(channel(0), channel(1), channel(2), channel(3))
}

pub fn gray(color: Color32) -> Color32 {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let y = ((r as u32 * 54 + g as u32 * 183 + b as u32 * 19) / 256) as u8;
    Color32::from_rgba_unmultiplied(y, y, y, a)
}

pub fn fill(ctx: &egui::Context, rgb: [u8; 3], installed: bool, progress: f32) -> Color32 {
    let base = super::palette(ctx).0;
    mix(
        base,
        super::accent(rgb),
        if installed { 0.035 } else { 0.0 } + 0.018 * progress,
    )
}

pub fn accent(ctx: &egui::Context, rgb: [u8; 3]) -> Color32 {
    let mut color = super::accent(rgb);
    let background = super::palette(ctx).0;
    // Small identity text remains readable in both themes, including yellow/cyan.
    for _ in 0..24 {
        let a = luminance(color);
        let b = luminance(background);
        if (a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5 {
            break;
        }
        color = mix(
            color,
            if ctx.style().visuals.dark_mode {
                Color32::WHITE
            } else {
                Color32::BLACK
            },
            0.1,
        );
    }
    color
}

fn luminance(color: Color32) -> f32 {
    let linear = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

pub fn line(painter: &egui::Painter, rect: Rect, color: Color32, progress: f32) {
    if progress > 0.0 {
        painter.hline(
            rect.left()..=rect.left() + rect.width() * progress,
            rect.top() + 1.0,
            Stroke::new(2.0_f32, color),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animation_advances_then_retracts_without_overshoot_and_respects_reduced_motion() {
        let ctx = egui::Context::default();
        let mut values = Vec::new();
        for frame in 0..40 {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(frame as f64 / 60.0),
                    ..Default::default()
                },
                |ctx| {
                    values.push(progress(
                        ctx,
                        "stable-id",
                        "grid",
                        (2..22).contains(&frame),
                        false,
                    ));
                    assert_eq!(progress(ctx, "other-app", "grid", false, false), 0.0);
                },
            );
        }
        assert_eq!(values[0], 0.0);
        assert!(values[2..14].iter().any(|&t| t > 0.05 && t < 0.95));
        assert!(values[2..16].windows(2).all(|v| v[1] >= v[0]));
        assert_eq!(values[20], 1.0);
        assert!(values[22..].windows(2).all(|v| v[1] <= v[0]));
        assert_eq!(*values.last().unwrap(), 0.0);
        let _ = ctx.run(Default::default(), |ctx| {
            assert_eq!(progress(ctx, "reduced", "grid", true, true), 1.0);
            assert_eq!(progress(ctx, "reduced", "grid", false, true), 0.0);
        });
        let slot = Rect::from_min_size(egui::pos2(16.0, 49.0), egui::vec2(54.0, 54.0));
        let zoom = logo_rect(slot, 1.0, false);
        assert_eq!(zoom.center(), slot.center());
        assert!((zoom.width() / slot.width() - 1.04).abs() < 0.001);
        assert_eq!(logo_rect(slot, 1.0, true), slot);
    }

    #[test]
    fn neutral_rest_subtle_installed_fill_and_readable_accents_in_both_themes() {
        let ctx = egui::Context::default();
        for dark in [true, false] {
            super::super::apply_mode(&ctx, dark);
            let base = super::super::palette(&ctx).0;
            for rgb in [[255, 190, 0], [0, 200, 255], [170, 110, 255]] {
                assert_eq!(fill(&ctx, rgb, false, 0.0), base);
                let installed = fill(&ctx, rgb, true, 0.0);
                assert!((installed.r() as i16 - base.r() as i16).abs() <= 9);
                let color = accent(&ctx, rgb);
                let a = luminance(color);
                let b = luminance(base);
                assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
            }
            let gray = gray(Color32::from_rgba_unmultiplied(200, 40, 80, 128));
            assert_eq!(gray.r(), gray.g());
            assert_eq!(gray.g(), gray.b());
            assert_eq!(gray.a(), 128);
        }
    }
}
