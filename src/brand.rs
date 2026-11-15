//! Studio App native window mark; matches assets/brand/studio-app.svg.
pub fn icon_data() -> eframe::egui::IconData {
    let width = 64u32;
    let height = 64u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let px = ((x as f32 + 0.5) * 4.0, (y as f32 + 0.5) * 4.0);
            let rounded = |l: f32, t: f32, r: f32, b: f32, radius: f32| {
                let cx = px.0.clamp(l + radius, r - radius);
                let cy = px.1.clamp(t + radius, b - radius);
                (px.0 - cx).powi(2) + (px.1 - cy).powi(2) <= radius.powi(2)
            };
            let rgba_pixel = if rounded(0.0, 0.0, 256.0, 256.0, 54.0) {
                if rounded(34.0, 48.0, 222.0, 90.0, 12.0) {
                    [162, 104, 232, 255]
                } else if rounded(34.0, 110.0, 112.0, 208.0, 12.0) {
                    [52, 144, 242, 255]
                } else if rounded(134.0, 110.0, 222.0, 208.0, 12.0) {
                    [127, 95, 229, 255]
                } else {
                    [16, 17, 23, 255]
                }
            } else {
                [0, 0, 0, 0]
            };
            let i = ((y * width + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&rgba_pixel);
        }
    }
    eframe::egui::IconData {
        rgba,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_is_valid_rgba() {
        let icon = super::icon_data();
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert_eq!(&icon.rgba[..4], &[0, 0, 0, 0]);
        assert!(icon.rgba.chunks_exact(4).any(|p| p == [162, 104, 232, 255]));
    }
}
