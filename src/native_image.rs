pub fn redact(image: &mut RgbImage, screen: Bounds, windows: &[Window], protected: &[String]) {
    for window in windows.iter().filter(|window| {
        protected
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&window.process))
    }) {
        let rect = window.bounds;
        let pad = 8;
        let x0 = (rect.x - screen.x - pad).max(0) as u32;
        let y0 = (rect.y - screen.y - pad).max(0) as u32;
        let x1 = (rect.x as i64 + rect.width as i64 - screen.x as i64 + pad as i64)
            .clamp(0, image.width() as i64) as u32;
        let y1 = (rect.y as i64 + rect.height as i64 - screen.y as i64 + pad as i64)
            .clamp(0, image.height() as i64) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                image.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
    }
}
pub fn compose(images: &[RgbImage], horizontal: bool) -> Result<RgbImage> {
    anyhow::ensure!(
        !images.is_empty() && images.len() <= 8,
        "画面を1〜8枚選んでください。"
    );
    let columns = if horizontal {
        images.len() as u32
    } else {
        (images.len() as f64).sqrt().ceil() as u32
    };
    let rows = (images.len() as u32).div_ceil(columns);
    let cell_w = 1920 / columns;
    let cell_h = 1080 / rows;
    let mut output = RgbImage::from_pixel(1920, 1080, Rgb([17, 26, 29]));
    for (index, image) in images.iter().enumerate() {
        let scale =
            (cell_w as f64 / image.width() as f64).min(cell_h as f64 / image.height() as f64);
        let w = (image.width() as f64 * scale).round().max(1.0) as u32;
        let h = (image.height() as f64 * scale).round().max(1.0) as u32;
        let tile = imageops::resize(image, w.min(cell_w), h.min(cell_h), FilterType::Triangle);
        imageops::replace(
            &mut output,
            &tile,
            ((index as u32 % columns) * cell_w + (cell_w - tile.width()) / 2) as i64,
            ((index as u32 / columns) * cell_h + (cell_h - tile.height()) / 2) as i64,
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privacy_handles_negative_screen_coordinates() {
        let mut image = RgbImage::from_pixel(100, 100, Rgb([255, 255, 255]));
        redact(
            &mut image,
            Bounds {
                x: -100,
                y: 0,
                width: 100,
                height: 100,
            },
            &[Window {
                process: "Secret".into(),
                title: String::new(),
                bounds: Bounds {
                    x: -60,
                    y: 20,
                    width: 40,
                    height: 40,
                },
            }],
            &["secret".into()],
        );
        assert_eq!(*image.get_pixel(50, 40), Rgb([0, 0, 0]));
        assert_eq!(*image.get_pixel(0, 0), Rgb([255, 255, 255]));
    }
}
