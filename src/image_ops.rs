//! Image processing operations: loading, cropping, and high-quality resampling.
//!
//! Preserves original image data and performs all edits on derived representations.
//! Coordinate spaces are strictly distinguished:
//! - Normalized crop space (`[0.0, 1.0]`)
//! - Source pixel space (`[0, width]`, `[0, height]`)
//! - Output print pixel space determined by physical millimeters and target DPI.

use ::image::{DynamicImage, GenericImageView, ImageError, RgbaImage, imageops::FilterType};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors arising during image operations.
#[derive(Debug, Error)]
pub enum ImageOpsError {
    #[error("Failed to decode or read image: {0}")]
    Image(#[from] ImageError),

    #[error("I/O error reading image file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid crop area dimensions")]
    InvalidCropArea,

    #[error("Invalid target pixel dimensions: {0}x{1}")]
    InvalidTargetDimensions(u32, u32),
}

/// Normalized crop rectangle relative to source image dimensions (values in `0.0..=1.0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for CropArea {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}

impl CropArea {
    /// Creates a new crop area with normalized coordinates.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        let mut area = Self {
            x,
            y,
            width,
            height,
        };
        area.clamp_within_bounds();
        area
    }

    /// Computes the largest centered crop rectangle that matches `target_aspect` (width / height)
    /// within a source image of size `(source_w, source_h)`.
    pub fn default_for_aspect(source_w: u32, source_h: u32, target_aspect: f32) -> Self {
        if source_w == 0 || source_h == 0 || target_aspect <= 0.0 || !target_aspect.is_finite() {
            return Self::default();
        }

        let source_aspect = source_w as f32 / source_h as f32;

        if source_aspect > target_aspect {
            // Source is wider than target aspect ratio -> fit full height, crop horizontal sides
            let crop_w = (target_aspect / source_aspect).clamp(0.01, 1.0);
            let crop_x = ((1.0 - crop_w) / 2.0).clamp(0.0, 1.0);
            Self {
                x: crop_x,
                y: 0.0,
                width: crop_w,
                height: 1.0,
            }
        } else {
            // Source is taller than target aspect ratio -> fit full width, crop vertical sides
            let crop_h = (source_aspect / target_aspect).clamp(0.01, 1.0);
            let crop_y = ((1.0 - crop_h) / 2.0).clamp(0.0, 1.0);
            Self {
                x: 0.0,
                y: crop_y,
                width: 1.0,
                height: crop_h,
            }
        }
    }

    /// Adjusts this crop area to match a new `target_aspect` ratio while preserving the current center.
    pub fn adjust_aspect(&mut self, source_w: u32, source_h: u32, target_aspect: f32) {
        if source_w == 0 || source_h == 0 || target_aspect <= 0.0 || !target_aspect.is_finite() {
            return;
        }

        let center_x = self.x + (self.width / 2.0);
        let center_y = self.y + (self.height / 2.0);

        let source_aspect = source_w as f32 / source_h as f32;
        // In source pixel terms: crop_pixel_w / crop_pixel_h = target_aspect
        // crop_pixel_w = crop.width * source_w
        // crop_pixel_h = crop.height * source_h
        // (crop.width * source_w) / (crop.height * source_h) = target_aspect
        // crop.width / crop.height = target_aspect / source_aspect = desired_normalized_aspect
        let desired_norm_aspect = target_aspect / source_aspect;

        let cur_size = (self.width * self.height).sqrt().clamp(0.1, 1.0);

        let mut new_w = cur_size * desired_norm_aspect.sqrt();
        let mut new_h = cur_size / desired_norm_aspect.sqrt();

        if new_w > 1.0 {
            new_w = 1.0;
            new_h = 1.0 / desired_norm_aspect;
        }
        if new_h > 1.0 {
            new_h = 1.0;
            new_w = desired_norm_aspect;
        }

        let new_w = new_w.clamp(0.05, 1.0);
        let new_h = new_h.clamp(0.05, 1.0);

        self.width = new_w;
        self.height = new_h;
        self.x = (center_x - new_w / 2.0).clamp(0.0, (1.0 - new_w).max(0.0));
        self.y = (center_y - new_h / 2.0).clamp(0.0, (1.0 - new_h).max(0.0));
        self.clamp_within_bounds();
    }

    /// Clamps normalized values so the crop box remains entirely within `[0.0, 1.0]`.
    pub fn clamp_within_bounds(&mut self) {
        self.width = self.width.clamp(0.01, 1.0);
        self.height = self.height.clamp(0.01, 1.0);
        self.x = self.x.clamp(0.0, (1.0 - self.width).max(0.0));
        self.y = self.y.clamp(0.0, (1.0 - self.height).max(0.0));
    }

    /// Converts normalized crop coordinates into discrete source pixel coordinates.
    pub fn to_pixel_rect(&self, source_w: u32, source_h: u32) -> (u32, u32, u32, u32) {
        if source_w == 0 || source_h == 0 {
            return (0, 0, 0, 0);
        }

        let px_x = ((self.x * source_w as f32).round() as u32).min(source_w.saturating_sub(1));
        let px_y = ((self.y * source_h as f32).round() as u32).min(source_h.saturating_sub(1));

        let max_w = source_w - px_x;
        let max_h = source_h - px_y;

        let px_w = ((self.width * source_w as f32).round() as u32)
            .max(1)
            .min(max_w);
        let px_h = ((self.height * source_h as f32).round() as u32)
            .max(1)
            .min(max_h);

        (px_x, px_y, px_w, px_h)
    }
}

/// Represents an imported original photograph.
#[derive(Clone)]
pub struct LoadedImage {
    pub file_path: Option<PathBuf>,
    pub file_name: String,
    pub original: DynamicImage,
    pub width: u32,
    pub height: u32,
}

impl LoadedImage {
    /// Loads an image from a file system path.
    pub fn from_path(path: &Path) -> Result<Self, ImageOpsError> {
        let original = ::image::open(path)?;
        let (width, height) = original.dimensions();
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        Ok(Self {
            file_path: Some(path.to_path_buf()),
            file_name,
            original,
            width,
            height,
        })
    }

    /// Loads an image from raw memory bytes.
    pub fn from_bytes(bytes: &[u8], name: &str) -> Result<Self, ImageOpsError> {
        let original = ::image::load_from_memory(bytes)?;
        let (width, height) = original.dimensions();

        Ok(Self {
            file_path: None,
            file_name: name.to_string(),
            original,
            width,
            height,
        })
    }

    /// Creates a loaded image directly from an existing `DynamicImage`.
    pub fn from_dynamic_image(original: DynamicImage, name: &str) -> Self {
        let (width, height) = original.dimensions();
        Self {
            file_path: None,
            file_name: name.to_string(),
            original,
            width,
            height,
        }
    }

    /// Aspect ratio of the original source image (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        if self.height > 0 {
            self.width as f32 / self.height as f32
        } else {
            1.0
        }
    }
}

/// Crops a region from the original image and resamples it using high-quality Lanczos3 filter
/// to exact discrete target pixel dimensions.
pub fn crop_and_resample(
    original: &DynamicImage,
    crop: &CropArea,
    target_width_px: u32,
    target_height_px: u32,
) -> Result<RgbaImage, ImageOpsError> {
    if target_width_px == 0 || target_height_px == 0 {
        return Err(ImageOpsError::InvalidTargetDimensions(
            target_width_px,
            target_height_px,
        ));
    }

    let (src_w, src_h) = original.dimensions();
    if src_w == 0 || src_h == 0 {
        return Err(ImageOpsError::InvalidCropArea);
    }

    let (px_x, px_y, px_w, px_h) = crop.to_pixel_rect(src_w, src_h);

    // Extract crop sub-image without mutating original
    let sub = ::image::imageops::crop_imm(original, px_x, px_y, px_w, px_h);

    // Resample to exact target pixel dimensions using high-quality Lanczos3 filter
    let resized = ::image::imageops::resize(
        &sub.to_image(),
        target_width_px,
        target_height_px,
        FilterType::Lanczos3,
    );

    Ok(resized)
}

/// Converts an RGBA image into an `egui::ColorImage` for GPU rendering in egui.
pub fn to_egui_color_image(img: &RgbaImage) -> eframe::egui::ColorImage {
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.as_raw();
    eframe::egui::ColorImage::from_rgba_unmultiplied(size, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_area_aspect_ratio_wide_image() {
        // Source: 1000 x 500 (aspect 2.0). Target aspect: 1.0 (square passport)
        let crop = CropArea::default_for_aspect(1000, 500, 1.0);
        // Crop should take full height (1.0), and half the width (0.5), centered at 0.25
        assert_eq!(crop.height, 1.0);
        assert!((crop.width - 0.5).abs() < 1e-4);
        assert!((crop.x - 0.25).abs() < 1e-4);
        assert_eq!(crop.y, 0.0);

        let (px_x, px_y, px_w, px_h) = crop.to_pixel_rect(1000, 500);
        assert_eq!(px_x, 250);
        assert_eq!(px_y, 0);
        assert_eq!(px_w, 500);
        assert_eq!(px_h, 500);
    }

    #[test]
    fn test_crop_area_aspect_ratio_tall_image() {
        // Source: 500 x 1000 (aspect 0.5). Target aspect: 1.0 (square passport)
        let crop = CropArea::default_for_aspect(500, 1000, 1.0);
        // Crop should take full width (1.0), and half the height (0.5), centered vertically at 0.25
        assert_eq!(crop.width, 1.0);
        assert!((crop.height - 0.5).abs() < 1e-4);
        assert_eq!(crop.x, 0.0);
        assert!((crop.y - 0.25).abs() < 1e-4);

        let (px_x, px_y, px_w, px_h) = crop.to_pixel_rect(500, 1000);
        assert_eq!(px_x, 0);
        assert_eq!(px_y, 250);
        assert_eq!(px_w, 500);
        assert_eq!(px_h, 500);
    }

    #[test]
    fn test_crop_and_resample_output_dimensions() {
        let original = DynamicImage::new_rgb8(800, 600);
        let crop = CropArea::default();
        let result = crop_and_resample(&original, &crop, 600, 600);
        assert!(result.is_ok());
        let res_img = result.unwrap();
        assert_eq!(res_img.width(), 600);
        assert_eq!(res_img.height(), 600);
    }

    #[test]
    fn test_adjust_aspect_preserves_center() {
        let mut crop = CropArea::default();
        // Adjust for a 35x45 mm passport (aspect 35/45 = ~0.777)
        crop.adjust_aspect(1000, 1000, 35.0 / 45.0);
        let center_x = crop.x + (crop.width / 2.0);
        let center_y = crop.y + (crop.height / 2.0);
        assert!((center_x - 0.5).abs() < 1e-3);
        assert!((center_y - 0.5).abs() < 1e-3);
    }
}
