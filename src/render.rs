use anyhow::{Context, Result, bail, ensure};
use image::{Rgba, RgbaImage, imageops};
use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const WIDTH: u32 = 1600;
pub const HEIGHT: u32 = 900;

#[derive(Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub group: String,
    pub asset: String,
    pub background: String,
    pub foreground: Option<String>,
    pub scale: f32,
    pub source: String,
}

pub fn color(hex: &str) -> Result<[u8; 3]> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    ensure!(
        hex.len() == 6 && hex.is_ascii(),
        "Use a six-digit color, e.g. #203C50"
    );
    Ok([
        u8::from_str_radix(&hex[0..2], 16)?,
        u8::from_str_radix(&hex[2..4], 16)?,
        u8::from_str_radix(&hex[4..6], 16)?,
    ])
}

pub fn load_mark(path: &Path) -> Result<RgbaImage> {
    let bytes = std::fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let raster = if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("svg"))
    {
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        let tree = usvg::Tree::from_data(&bytes, &options)?;
        let size = tree.size();
        let factor = 1400.0 / size.width().max(size.height());
        let mut pixmap = tiny_skia::Pixmap::new(
            (size.width() * factor).ceil().max(1.0) as u32,
            (size.height() * factor).ceil().max(1.0) as u32,
        )
        .context("Cannot allocate SVG raster")?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(factor, factor),
            &mut pixmap.as_mut(),
        );
        // PNG decoding converts tiny-skia's premultiplied pixels to straight RGBA.
        image::load_from_memory(&pixmap.encode_png()?)?.to_rgba8()
    } else {
        image::load_from_memory(&bytes)?.to_rgba8()
    };
    trim(&raster)
}

fn trim(image: &RgbaImage) -> Result<RgbaImage> {
    let (mut left, mut top, mut right, mut bottom) = (image.width(), image.height(), 0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if p[3] != 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    ensure!(
        right > left && bottom > top,
        "The image is fully transparent"
    );
    Ok(imageops::crop_imm(image, left, top, right - left, bottom - top).to_image())
}

pub fn compose(mark: &RgbaImage, preset: &Preset) -> Result<RgbaImage> {
    ensure!(
        preset.scale.is_finite() && (0.4..=1.6).contains(&preset.scale),
        "Scale must be between 0.4 and 1.6"
    );
    let [r, g, b] = color(&preset.background)?;
    let mut canvas = RgbaImage::from_pixel(WIDTH, HEIGHT, Rgba([r, g, b, 255]));
    let mut mark = mark.clone();
    // Only generic icons have a foreground override; brand marks keep their original colors.
    if let Some(foreground) = &preset.foreground {
        let [r, g, b] = color(foreground)?;
        for pixel in mark.pixels_mut() {
            pixel.0 = [r, g, b, pixel[3]];
        }
    }
    let factor = (960.0 * preset.scale / mark.width() as f32)
        .min(390.0 * preset.scale / mark.height() as f32);
    let width = (mark.width() as f32 * factor).round().max(1.0) as u32;
    let height = (mark.height() as f32 * factor).round().max(1.0) as u32;
    let scaled = imageops::resize(&mark, width, height, imageops::FilterType::Lanczos3);
    imageops::overlay(
        &mut canvas,
        &scaled,
        ((WIDTH - width) / 2) as i64,
        ((HEIGHT - height) / 2) as i64,
    );
    Ok(canvas)
}

pub fn export(root: &Path, output: &Path, preset: &Preset) -> Result<()> {
    if preset.id.is_empty()
        || !preset
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        bail!("Invalid output filename: {}", preset.id);
    }
    let mark = load_mark(&root.join(&preset.asset))?;
    let canvas = compose(&mark, preset)?;
    std::fs::create_dir_all(output)?;
    canvas.save(output.join(format!("{}.png", preset.id)))?;
    Ok(())
}

pub fn export_all(root: &Path, output: &Path, presets: &[Preset]) -> Result<()> {
    for preset in presets {
        export(root, output, preset).with_context(|| format!("Export failed: {}", preset.name))?;
    }
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_string_pretty(presets)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_padding_is_removed_and_mark_is_centered_without_distortion() {
        let mut source = RgbaImage::new(100, 80);
        for y in 20..40 {
            for x in 10..50 {
                source.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let mark = trim(&source).unwrap();
        assert_eq!(mark.dimensions(), (40, 20));
        let p = Preset {
            id: "test".into(),
            name: "test".into(),
            group: "".into(),
            asset: "".into(),
            background: "#102030".into(),
            foreground: None,
            scale: 1.0,
            source: "".into(),
        };
        let image = compose(&mark, &p).unwrap();
        assert_eq!(image.dimensions(), (1600, 900));
        assert_eq!(image.get_pixel(0, 0).0, [16, 32, 48, 255]);
        assert_eq!(image.get_pixel(800, 450).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(409, 450).0, [16, 32, 48, 255]);
        assert_eq!(image.get_pixel(410, 450).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(1189, 450).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(1190, 450).0, [16, 32, 48, 255]);
    }
    #[test]
    fn rejects_empty_images_and_bad_colors() {
        assert!(trim(&RgbaImage::new(5, 5)).is_err());
        assert!(color("#xyzxyz").is_err());
        assert!(color("日本").is_err());
    }
}
