use anyhow::{Result, ensure};
use image::RgbImage;

#[derive(Clone, Copy)]
pub enum PixelFormat {
    Rgb,
    Rgbx,
    Bgr,
    Bgrx,
}

// PipeWire frames can have row padding and a nonzero chunk offset. Never
// interpret the entire mapped allocation as tightly packed image pixels.
pub fn decode(
    bytes: &[u8],
    width: u32,
    height: u32,
    offset: usize,
    stride: i32,
    format: PixelFormat,
) -> Result<RgbImage> {
    ensure!(
        width > 0 && height > 0 && width <= 16384 && height <= 16384,
        "画面サイズが不正です。"
    );
    let channels = match format {
        PixelFormat::Rgb | PixelFormat::Bgr => 3,
        _ => 4,
    };
    let row = width as usize * channels;
    let size = width as usize * height as usize * 3;
    ensure!(size <= 256 * 1024 * 1024, "画面サイズが大きすぎます。");
    let stride = if stride == 0 {
        row as i64
    } else {
        stride as i64
    };
    ensure!(
        stride.unsigned_abs() >= row as u64,
        "画面の行幅が不正です。"
    );
    let mut output = vec![0; size];
    for y in 0..height as usize {
        let start = offset as i64 + stride * y as i64;
        ensure!(
            start >= 0
                && (start as usize)
                    .checked_add(row)
                    .is_some_and(|end| end <= bytes.len()),
            "画面バッファーが不足しています。"
        );
        let source = &bytes[start as usize..start as usize + row];
        let target = &mut output[y * width as usize * 3..(y + 1) * width as usize * 3];
        for (source, target) in source
            .chunks_exact(channels)
            .zip(target.chunks_exact_mut(3))
        {
            match format {
                PixelFormat::Rgb | PixelFormat::Rgbx => target.copy_from_slice(&source[..3]),
                PixelFormat::Bgr | PixelFormat::Bgrx => {
                    target.copy_from_slice(&[source[2], source[1], source[0]])
                }
            }
        }
    }
    Ok(RgbImage::from_raw(width, height, output).expect("validated image size"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_bgrx_with_chunk_offset() {
        let bytes = [
            99, 99, 3, 2, 1, 0, 6, 5, 4, 0, 99, 99, 99, 99, 9, 8, 7, 0, 12, 11, 10, 0,
        ];
        let image = decode(&bytes, 2, 2, 2, 12, PixelFormat::Bgrx).unwrap();
        assert_eq!(image.into_raw(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }

    #[test]
    fn inverted_rgb_rows() {
        assert_eq!(
            decode(&[1, 2, 3, 4, 5, 6], 1, 2, 3, -3, PixelFormat::Rgb)
                .unwrap()
                .into_raw(),
            [4, 5, 6, 1, 2, 3]
        );
    }

    #[test]
    fn rejects_truncated_or_invalid_frames() {
        assert!(decode(&[0; 7], 2, 1, 0, 8, PixelFormat::Rgbx).is_err());
        assert!(decode(&[0; 8], 2, 1, 0, 4, PixelFormat::Bgr).is_err());
        assert!(decode(&[], u32::MAX, 2, 0, 0, PixelFormat::Rgb).is_err());
        assert!(decode(&[0; 6], 1, 2, 0, -3, PixelFormat::Rgb).is_err());
    }
}
