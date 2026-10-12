//! Export engine for print-ready PDF documents and raster images (PNG, JPEG).
//!
//! Generates final print outputs strictly from underlying image data at physical dimensions
//! and target DPI, completely independent of preview GUI coordinates.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use ::image::{DynamicImage, ImageError, Rgb, RgbImage, RgbaImage};
use printpdf::{
    Color, Greyscale, Line, LinePoint, Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Point, Pt,
    RawImage, XObjectTransform,
};
use thiserror::Error;

use crate::image_ops::{CropArea, crop_and_resample};
use crate::layout::{GridConfig, LayoutError, compute_layout};
use crate::units::Dpi;

/// Errors arising during export operations.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("I/O error during export: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image encoding error: {0}")]
    Image(#[from] ImageError),

    #[error("PDF generation failure: {0}")]
    Pdf(String),

    #[error("Layout configuration error: {0}")]
    Layout(#[from] LayoutError),

    #[error("Image processing error: {0}")]
    ImageOps(#[from] crate::image_ops::ImageOpsError),

    #[error("Unsupported export file extension: {0}")]
    UnsupportedExtension(String),
}

/// Target output format for export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Png,
    Jpeg,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Png => "png",
            ExportFormat::Jpeg => "jpg",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "PDF Document (*.pdf)",
            ExportFormat::Png => "PNG Image (*.png)",
            ExportFormat::Jpeg => "JPEG Image (*.jpg, *.jpeg)",
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "pdf" => Some(ExportFormat::Pdf),
            "png" => Some(ExportFormat::Png),
            "jpg" | "jpeg" => Some(ExportFormat::Jpeg),
            _ => None,
        }
    }
}

/// What to export: full sheet grid layout or single photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    FullSheet,
    SinglePhoto,
}

impl ExportTarget {
    pub fn label(self) -> &'static str {
        match self {
            ExportTarget::FullSheet => "Full Sheet (Grid Layout)",
            ExportTarget::SinglePhoto => "Single Passport Photo",
        }
    }
}

/// Summary report returned upon successful export.
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub path: PathBuf,
    pub target: ExportTarget,
    pub format: ExportFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: Dpi,
    pub file_size_bytes: u64,
}

/// Exports the full printable sheet grid layout to an RGB raster image at exact DPI.
pub fn export_sheet_to_image(
    config: &GridConfig,
    dpi: Dpi,
    original: &DynamicImage,
    crop: &CropArea,
    show_cut_marks: bool,
) -> Result<RgbImage, ExportError> {
    let layout = compute_layout(config)?;

    let (sheet_w_px, sheet_h_px) = config.sheet.to_pixels(dpi);
    let (photo_w_px, photo_h_px) = config.photo.to_pixels(dpi);

    // Resample cropped photo once at exact target output pixels
    let photo_rgba = crop_and_resample(original, crop, photo_w_px.0, photo_h_px.0)?;

    // White paper background
    let mut sheet_img = RgbImage::from_pixel(sheet_w_px.0, sheet_h_px.0, Rgb([255, 255, 255]));

    // Place each photo slot onto the sheet image
    for slot in &layout.slots {
        let (slot_x_px, slot_y_px, _, _) = slot.to_pixel_rect(dpi);
        let x_origin = slot_x_px.0;
        let y_origin = slot_y_px.0;

        for py in 0..photo_h_px.0 {
            for px in 0..photo_w_px.0 {
                let dest_x = x_origin + px;
                let dest_y = y_origin + py;

                if dest_x < sheet_w_px.0 && dest_y < sheet_h_px.0 {
                    let pixel = photo_rgba.get_pixel(px, py);
                    // Composite over white
                    let alpha = pixel[3] as f32 / 255.0;
                    let r = ((pixel[0] as f32 * alpha) + (255.0 * (1.0 - alpha))).round() as u8;
                    let g = ((pixel[1] as f32 * alpha) + (255.0 * (1.0 - alpha))).round() as u8;
                    let b = ((pixel[2] as f32 * alpha) + (255.0 * (1.0 - alpha))).round() as u8;
                    sheet_img.put_pixel(dest_x, dest_y, Rgb([r, g, b]));
                }
            }
        }

        // Corner cut marks
        if show_cut_marks {
            let mark_len = ((2.0 / 25.4) * dpi.0 as f32).round().max(4.0) as u32;
            let mark_color = Rgb([80, 80, 80]);

            draw_cut_marks(
                &mut sheet_img,
                x_origin,
                y_origin,
                photo_w_px.0,
                photo_h_px.0,
                mark_len,
                mark_color,
            );
        }
    }

    Ok(sheet_img)
}

/// Helper to draw corner cut tick marks on a raster sheet image.
fn draw_cut_marks(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, len: u32, color: Rgb<u8>) {
    let (max_w, max_h) = img.dimensions();

    let right = x + w;
    let bottom = y + h;

    // Top-left
    for d in 0..len {
        if x + d < max_w && y < max_h {
            img.put_pixel(x + d, y, color);
        }
        if x < max_w && y + d < max_h {
            img.put_pixel(x, y + d, color);
        }
    }

    // Top-right
    for d in 0..len {
        if right.saturating_sub(d + 1) < max_w && y < max_h {
            img.put_pixel(right.saturating_sub(d + 1), y, color);
        }
        if right.saturating_sub(1) < max_w && y + d < max_h {
            img.put_pixel(right.saturating_sub(1), y + d, color);
        }
    }

    // Bottom-left
    for d in 0..len {
        if x + d < max_w && bottom.saturating_sub(1) < max_h {
            img.put_pixel(x + d, bottom.saturating_sub(1), color);
        }
        if x < max_w && bottom.saturating_sub(d + 1) < max_h {
            img.put_pixel(x, bottom.saturating_sub(d + 1), color);
        }
    }

    // Bottom-right
    for d in 0..len {
        if right.saturating_sub(d + 1) < max_w && bottom.saturating_sub(1) < max_h {
            img.put_pixel(right.saturating_sub(d + 1), bottom.saturating_sub(1), color);
        }
        if right.saturating_sub(1) < max_w && bottom.saturating_sub(d + 1) < max_h {
            img.put_pixel(right.saturating_sub(1), bottom.saturating_sub(d + 1), color);
        }
    }
}

/// Exports the full printable sheet grid layout to a vector PDF document with embedded images.
pub fn export_sheet_to_pdf(
    config: &GridConfig,
    dpi: Dpi,
    original: &DynamicImage,
    crop: &CropArea,
    show_cut_marks: bool,
) -> Result<Vec<u8>, ExportError> {
    let layout = compute_layout(config)?;

    let (photo_w_px, photo_h_px) = config.photo.to_pixels(dpi);
    let photo_rgba = crop_and_resample(original, crop, photo_w_px.0, photo_h_px.0)?;

    let mut doc = PdfDocument::new("Imprint Photo Print Layout");

    let raw_image = RawImage::from_dynamic_image(DynamicImage::ImageRgba8(photo_rgba))
        .map_err(ExportError::Pdf)?;
    let image_id = doc.add_image(&raw_image);

    let mut ops = Vec::new();

    let sheet_h_mm = config.sheet.height.0;

    // In PDF coordinate space, (0, 0) is at the bottom-left of the page.
    // slot.x is offset from sheet left in mm.
    // slot.y is offset from sheet top in mm.
    // The bottom-left corner of the photo in PDF points:
    for slot in &layout.slots {
        let slot_left_mm = slot.x.0;
        let slot_bottom_mm = sheet_h_mm - (slot.y.0 + slot.height.0);

        let pt_x = Pt(slot_left_mm / 25.4 * 72.0);
        let pt_y = Pt(slot_bottom_mm / 25.4 * 72.0);

        let transform = XObjectTransform {
            translate_x: Some(pt_x),
            translate_y: Some(pt_y),
            dpi: Some(dpi.0 as f32),
            ..Default::default()
        };

        ops.push(Op::UseXobject {
            id: image_id.clone(),
            transform,
        });

        // Add corner cut marks if requested
        if show_cut_marks {
            let mark_len_pt = (2.0 / 25.4) * 72.0;
            let slot_w_pt = (slot.width.0 / 25.4) * 72.0;
            let slot_h_pt = (slot.height.0 / 25.4) * 72.0;

            let x0 = pt_x.0;
            let y0 = pt_y.0;
            let x1 = x0 + slot_w_pt;
            let y1 = y0 + slot_h_pt;

            ops.push(Op::SetOutlineColor {
                col: Color::Greyscale(Greyscale {
                    percent: 0.3,
                    icc_profile: None,
                }),
            });
            ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

            // Corner ticks:
            // Bottom-left
            ops.push(make_pdf_line(x0, y0, x0 + mark_len_pt, y0));
            ops.push(make_pdf_line(x0, y0, x0, y0 + mark_len_pt));

            // Bottom-right
            ops.push(make_pdf_line(x1, y0, x1 - mark_len_pt, y0));
            ops.push(make_pdf_line(x1, y0, x1, y0 + mark_len_pt));

            // Top-left
            ops.push(make_pdf_line(x0, y1, x0 + mark_len_pt, y1));
            ops.push(make_pdf_line(x0, y1, x0, y1 - mark_len_pt));

            // Top-right
            ops.push(make_pdf_line(x1, y1, x1 - mark_len_pt, y1));
            ops.push(make_pdf_line(x1, y1, x1, y1 - mark_len_pt));
        }
    }

    let page = PdfPage::new(Mm(config.sheet.width.0), Mm(config.sheet.height.0), ops);
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    Ok(bytes)
}

/// Helper to create a straight line operation in PDF.
fn make_pdf_line(x1: f32, y1: f32, x2: f32, y2: f32) -> Op {
    Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point {
                        x: Pt(x1),
                        y: Pt(y1),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(x2),
                        y: Pt(y2),
                    },
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    }
}

/// Exports a single cropped photo to a raster image at exact physical dimensions and DPI.
pub fn export_single_photo_to_image(
    config: &GridConfig,
    dpi: Dpi,
    original: &DynamicImage,
    crop: &CropArea,
) -> Result<RgbaImage, ExportError> {
    let (photo_w_px, photo_h_px) = config.photo.to_pixels(dpi);
    let photo_rgba = crop_and_resample(original, crop, photo_w_px.0, photo_h_px.0)?;
    Ok(photo_rgba)
}

/// Exports a single cropped photo to a PDF page matching the exact physical photo size.
pub fn export_single_photo_to_pdf(
    config: &GridConfig,
    dpi: Dpi,
    original: &DynamicImage,
    crop: &CropArea,
) -> Result<Vec<u8>, ExportError> {
    let (photo_w_px, photo_h_px) = config.photo.to_pixels(dpi);
    let photo_rgba = crop_and_resample(original, crop, photo_w_px.0, photo_h_px.0)?;

    let mut doc = PdfDocument::new("Imprint Passport Photo");
    let raw_image = RawImage::from_dynamic_image(DynamicImage::ImageRgba8(photo_rgba))
        .map_err(ExportError::Pdf)?;
    let image_id = doc.add_image(&raw_image);

    let transform = XObjectTransform {
        translate_x: Some(Pt(0.0)),
        translate_y: Some(Pt(0.0)),
        dpi: Some(dpi.0 as f32),
        ..Default::default()
    };

    let page = PdfPage::new(
        Mm(config.photo.width.0),
        Mm(config.photo.height.0),
        vec![Op::UseXobject {
            id: image_id,
            transform,
        }],
    );
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    Ok(bytes)
}

/// Configuration options passed to [`export_to_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub dpi: Dpi,
    pub target: ExportTarget,
    pub format: Option<ExportFormat>,
    pub show_cut_marks: bool,
}

impl ExportOptions {
    pub fn new(
        dpi: Dpi,
        target: ExportTarget,
        format: Option<ExportFormat>,
        show_cut_marks: bool,
    ) -> Self {
        Self {
            dpi,
            target,
            format,
            show_cut_marks,
        }
    }
}

/// Master export dispatcher that writes output directly to the chosen file path.
pub fn export_to_file(
    path: &Path,
    config: &GridConfig,
    original: &DynamicImage,
    crop: &CropArea,
    options: ExportOptions,
) -> Result<ExportReport, ExportError> {
    let format = options
        .format
        .or_else(|| ExportFormat::from_path(path))
        .ok_or_else(|| {
            ExportError::UnsupportedExtension(
                path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("none")
                    .to_string(),
            )
        })?;

    let (width_px, height_px) = match options.target {
        ExportTarget::FullSheet => {
            let (w, h) = config.sheet.to_pixels(options.dpi);
            (w.0, h.0)
        }
        ExportTarget::SinglePhoto => {
            let (w, h) = config.photo.to_pixels(options.dpi);
            (w.0, h.0)
        }
    };

    match (options.target, format) {
        (ExportTarget::FullSheet, ExportFormat::Pdf) => {
            let bytes =
                export_sheet_to_pdf(config, options.dpi, original, crop, options.show_cut_marks)?;
            let mut file = File::create(path)?;
            file.write_all(&bytes)?;
        }
        (ExportTarget::FullSheet, ExportFormat::Png) => {
            let img =
                export_sheet_to_image(config, options.dpi, original, crop, options.show_cut_marks)?;
            img.save_with_format(path, ::image::ImageFormat::Png)?;
        }
        (ExportTarget::FullSheet, ExportFormat::Jpeg) => {
            let img =
                export_sheet_to_image(config, options.dpi, original, crop, options.show_cut_marks)?;
            img.save_with_format(path, ::image::ImageFormat::Jpeg)?;
        }
        (ExportTarget::SinglePhoto, ExportFormat::Pdf) => {
            let bytes = export_single_photo_to_pdf(config, options.dpi, original, crop)?;
            let mut file = File::create(path)?;
            file.write_all(&bytes)?;
        }
        (ExportTarget::SinglePhoto, ExportFormat::Png) => {
            let img = export_single_photo_to_image(config, options.dpi, original, crop)?;
            img.save_with_format(path, ::image::ImageFormat::Png)?;
        }
        (ExportTarget::SinglePhoto, ExportFormat::Jpeg) => {
            let img = export_single_photo_to_image(config, options.dpi, original, crop)?;
            // Convert RGBA to RGB for JPEG
            let rgb: RgbImage = DynamicImage::ImageRgba8(img).to_rgb8();
            rgb.save_with_format(path, ::image::ImageFormat::Jpeg)?;
        }
    }

    let file_size_bytes = std::fs::metadata(path)?.len();

    Ok(ExportReport {
        path: path.to_path_buf(),
        target: options.target,
        format,
        width_px,
        height_px,
        dpi: options.dpi,
        file_size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::DimensionsMm;

    fn make_test_image() -> DynamicImage {
        DynamicImage::new_rgb8(400, 400)
    }

    #[test]
    fn test_export_sheet_to_image_dimensions() {
        // 4x6 in = 101.6 x 152.4 mm
        // At 150 DPI:
        // 101.6 / 25.4 * 150 = 600 px
        // 152.4 / 25.4 * 150 = 900 px
        let config = GridConfig {
            sheet: DimensionsMm::new(101.6, 152.4),
            photo: DimensionsMm::new(50.8, 50.8),
            rows: 2,
            cols: 2,
            ..Default::default()
        };
        let dpi = Dpi(150);
        let img = make_test_image();
        let crop = CropArea::default();

        let sheet = export_sheet_to_image(&config, dpi, &img, &crop, true).unwrap();
        assert_eq!(sheet.width(), 600);
        assert_eq!(sheet.height(), 900);
    }

    #[test]
    fn test_export_single_photo_to_image_dimensions() {
        // 2x2 in = 50.8 x 50.8 mm
        // At 300 DPI: 600 x 600 px
        let config = GridConfig {
            sheet: DimensionsMm::new(101.6, 152.4),
            photo: DimensionsMm::new(50.8, 50.8),
            rows: 2,
            cols: 2,
            ..Default::default()
        };
        let dpi = Dpi::STANDARD;
        let img = make_test_image();
        let crop = CropArea::default();

        let single = export_single_photo_to_image(&config, dpi, &img, &crop).unwrap();
        assert_eq!(single.width(), 600);
        assert_eq!(single.height(), 600);
    }

    #[test]
    fn test_export_sheet_to_pdf_validity() {
        let config = GridConfig {
            sheet: DimensionsMm::new(101.6, 152.4),
            photo: DimensionsMm::new(50.8, 50.8),
            rows: 2,
            cols: 2,
            ..Default::default()
        };
        let dpi = Dpi(150);
        let img = make_test_image();
        let crop = CropArea::default();

        let pdf_bytes = export_sheet_to_pdf(&config, dpi, &img, &crop, true).unwrap();
        assert!(!pdf_bytes.is_empty());
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_export_single_photo_to_pdf_validity() {
        let config = GridConfig::default();
        let dpi = Dpi(150);
        let img = make_test_image();
        let crop = CropArea::default();

        let pdf_bytes = export_single_photo_to_pdf(&config, dpi, &img, &crop).unwrap();
        assert!(!pdf_bytes.is_empty());
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn test_export_overflow_error() {
        let config = GridConfig {
            sheet: DimensionsMm::new(50.0, 50.0),
            photo: DimensionsMm::new(100.0, 100.0),
            rows: 1,
            cols: 1,
            ..Default::default()
        };
        let dpi = Dpi::STANDARD;
        let img = make_test_image();
        let crop = CropArea::default();

        let result = export_sheet_to_image(&config, dpi, &img, &crop, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExportError::Layout(_)));
    }
}
