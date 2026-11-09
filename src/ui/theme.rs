use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(17, 18, 20);
pub const PANEL: Color32 = Color32::from_rgb(21, 22, 25);
pub const BORDER: Color32 = Color32::from_rgb(54, 55, 60);
pub const MUTED: Color32 = Color32::from_rgb(159, 160, 167);
pub const WHITE: Color32 = Color32::from_rgb(242, 242, 245);
pub const BLUE: Color32 = Color32::from_rgb(111, 184, 255);

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = BG;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = PANEL;
    style.visuals.override_text_color = Some(WHITE);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.inactive.weak_bg_fill = BG;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(38, 39, 45);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(48, 49, 57);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = 0.into();
    }
    style.visuals.window_corner_radius = 0.into();
    style.visuals.selection.bg_fill = Color32::from_rgb(63, 58, 83);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 11.0);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::new(13.0, FontFamily::Monospace));
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(27.0, FontFamily::Proportional),
    );
    ctx.set_style(style);
}

pub fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text).monospace().size(11.0).color(MUTED)
}
pub fn accent(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
pub fn tint(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(17 + rgb[0] / 14, 18 + rgb[1] / 14, 20 + rgb[2] / 14)
}

pub fn outline() -> egui::Frame {
    egui::Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(18)
}
