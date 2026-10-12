//! Print layout calculations, grid geometry, margins, gutters, and sheet fit validation.
//!
//! All calculations are performed in physical millimeters to ensure print accuracy
//! across preview and export pipelines.

use crate::units::{DimensionsMm, Dpi, Millimeters, Pixels};
use thiserror::Error;

/// Page margins in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarginsMm {
    pub left: Millimeters,
    pub right: Millimeters,
    pub top: Millimeters,
    pub bottom: Millimeters,
}

impl MarginsMm {
    pub const ZERO: Self = Self {
        left: Millimeters::ZERO,
        right: Millimeters::ZERO,
        top: Millimeters::ZERO,
        bottom: Millimeters::ZERO,
    };

    pub fn uniform(margin: f32) -> Self {
        Self {
            left: Millimeters(margin),
            right: Millimeters(margin),
            top: Millimeters(margin),
            bottom: Millimeters(margin),
        }
    }

    pub fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left: Millimeters(left),
            right: Millimeters(right),
            top: Millimeters(top),
            bottom: Millimeters(bottom),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.left.0.is_finite()
            && self.left.0 >= 0.0
            && self.right.0.is_finite()
            && self.right.0 >= 0.0
            && self.top.0.is_finite()
            && self.top.0 >= 0.0
            && self.bottom.0.is_finite()
            && self.bottom.0 >= 0.0
    }
}

/// Spacing between photos in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuttersMm {
    pub horizontal: Millimeters,
    pub vertical: Millimeters,
}

impl GuttersMm {
    pub const ZERO: Self = Self {
        horizontal: Millimeters::ZERO,
        vertical: Millimeters::ZERO,
    };

    pub fn uniform(spacing: f32) -> Self {
        Self {
            horizontal: Millimeters(spacing),
            vertical: Millimeters(spacing),
        }
    }

    pub fn new(horizontal: f32, vertical: f32) -> Self {
        Self {
            horizontal: Millimeters(horizontal),
            vertical: Millimeters(vertical),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.horizontal.0.is_finite()
            && self.horizontal.0 >= 0.0
            && self.vertical.0.is_finite()
            && self.vertical.0 >= 0.0
    }
}

/// Errors occurring during layout validation.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum LayoutError {
    #[error("Sheet dimensions ({sheet_w} x {sheet_h}) must be positive and non-zero", sheet_w = .0.width, sheet_h = .0.height)]
    InvalidSheetDimensions(DimensionsMm),

    #[error("Photo dimensions ({photo_w} x {photo_h}) must be positive and non-zero", photo_w = .0.width, photo_h = .0.height)]
    InvalidPhotoDimensions(DimensionsMm),

    #[error("Grid must contain at least 1 row and 1 column, got {rows} rows and {cols} columns")]
    ZeroGridCount { rows: u32, cols: u32 },

    #[error("Invalid margins (must be finite and non-negative)")]
    InvalidMargins,

    #[error("Invalid gutters (must be finite and non-negative)")]
    InvalidGutters,

    #[error("Margins ({left} mm left + {right} mm right) exceed sheet width {sheet_w} mm")]
    MarginsExceedSheetWidth { left: f32, right: f32, sheet_w: f32 },

    #[error("Margins ({top} mm top + {bottom} mm bottom) exceed sheet height {sheet_h} mm")]
    MarginsExceedSheetHeight { top: f32, bottom: f32, sheet_h: f32 },

    #[error("Grid width required ({required} mm) exceeds available sheet width ({available} mm)")]
    OverflowWidth {
        required: Millimeters,
        available: Millimeters,
    },

    #[error("Grid height required ({required} mm) exceeds available sheet height ({available} mm)")]
    OverflowHeight {
        required: Millimeters,
        available: Millimeters,
    },
}

/// Configuration parameters for generating a photo grid on a sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct GridConfig {
    pub sheet: DimensionsMm,
    pub photo: DimensionsMm,
    pub rows: u32,
    pub cols: u32,
    pub margins: MarginsMm,
    pub gutters: GuttersMm,
    pub auto_center: bool,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            // Standard 4x6 inch (101.6 x 152.4 mm) photo paper
            sheet: DimensionsMm::new(101.6, 152.4),
            // US Passport 2x2 in (50.8 x 50.8 mm)
            photo: DimensionsMm::new(50.8, 50.8),
            rows: 2,
            cols: 2,
            margins: MarginsMm::new(0.0, 0.0, 0.0, 0.0),
            gutters: GuttersMm::new(0.0, 0.0),
            auto_center: true,
        }
    }
}

/// Calculated placement for an individual photo item on the sheet in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoSlotMm {
    pub x: Millimeters,
    pub y: Millimeters,
    pub width: Millimeters,
    pub height: Millimeters,
    pub row: u32,
    pub col: u32,
}

impl PhotoSlotMm {
    /// Converts physical slot coordinates and dimensions to discrete pixel coordinates
    /// at the given output DPI using uniform rounding policy.
    pub fn to_pixel_rect(&self, dpi: Dpi) -> (Pixels, Pixels, Pixels, Pixels) {
        (
            self.x.to_pixels(dpi),
            self.y.to_pixels(dpi),
            self.width.to_pixels(dpi),
            self.height.to_pixels(dpi),
        )
    }
}

/// Resulting calculated geometry of the sheet layout.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutGeometry {
    pub slots: Vec<PhotoSlotMm>,
    pub grid_width: Millimeters,
    pub grid_height: Millimeters,
    pub printable_width: Millimeters,
    pub printable_height: Millimeters,
    pub origin_x: Millimeters,
    pub origin_y: Millimeters,
}

impl LayoutGeometry {
    pub fn count(&self) -> usize {
        self.slots.len()
    }
}

/// Computes the precise physical grid layout and validates bounds.
pub fn compute_layout(config: &GridConfig) -> Result<LayoutGeometry, LayoutError> {
    if !config.sheet.is_valid() {
        return Err(LayoutError::InvalidSheetDimensions(config.sheet));
    }
    if !config.photo.is_valid() {
        return Err(LayoutError::InvalidPhotoDimensions(config.photo));
    }
    if config.rows == 0 || config.cols == 0 {
        return Err(LayoutError::ZeroGridCount {
            rows: config.rows,
            cols: config.cols,
        });
    }
    if !config.margins.is_valid() {
        return Err(LayoutError::InvalidMargins);
    }
    if !config.gutters.is_valid() {
        return Err(LayoutError::InvalidGutters);
    }

    let h_margins = config.margins.left.0 + config.margins.right.0;
    if h_margins >= config.sheet.width.0 {
        return Err(LayoutError::MarginsExceedSheetWidth {
            left: config.margins.left.0,
            right: config.margins.right.0,
            sheet_w: config.sheet.width.0,
        });
    }

    let v_margins = config.margins.top.0 + config.margins.bottom.0;
    if v_margins >= config.sheet.height.0 {
        return Err(LayoutError::MarginsExceedSheetHeight {
            top: config.margins.top.0,
            bottom: config.margins.bottom.0,
            sheet_h: config.sheet.height.0,
        });
    }

    let printable_w = config.sheet.width.0 - h_margins;
    let printable_h = config.sheet.height.0 - v_margins;

    // Total required space for photos and gutters
    let required_w = (config.cols as f32) * config.photo.width.0
        + ((config.cols - 1) as f32) * config.gutters.horizontal.0;
    let required_h = (config.rows as f32) * config.photo.height.0
        + ((config.rows - 1) as f32) * config.gutters.vertical.0;

    // Tolerance of 0.01 mm for floating point rounding
    const EPSILON: f32 = 0.01;
    if required_w > printable_w + EPSILON {
        return Err(LayoutError::OverflowWidth {
            required: Millimeters(required_w),
            available: Millimeters(printable_w),
        });
    }
    if required_h > printable_h + EPSILON {
        return Err(LayoutError::OverflowHeight {
            required: Millimeters(required_h),
            available: Millimeters(printable_h),
        });
    }

    // Origin calculation (offset from sheet top-left)
    let (origin_x, origin_y) = if config.auto_center {
        let extra_x = (printable_w - required_w).max(0.0) / 2.0;
        let extra_y = (printable_h - required_h).max(0.0) / 2.0;
        (
            config.margins.left.0 + extra_x,
            config.margins.top.0 + extra_y,
        )
    } else {
        (config.margins.left.0, config.margins.top.0)
    };

    let mut slots = Vec::with_capacity((config.rows * config.cols) as usize);

    for r in 0..config.rows {
        for c in 0..config.cols {
            let x = origin_x + (c as f32) * (config.photo.width.0 + config.gutters.horizontal.0);
            let y = origin_y + (r as f32) * (config.photo.height.0 + config.gutters.vertical.0);

            slots.push(PhotoSlotMm {
                x: Millimeters(x),
                y: Millimeters(y),
                width: config.photo.width,
                height: config.photo.height,
                row: r,
                col: c,
            });
        }
    }

    Ok(LayoutGeometry {
        slots,
        grid_width: Millimeters(required_w),
        grid_height: Millimeters(required_h),
        printable_width: Millimeters(printable_w),
        printable_height: Millimeters(printable_h),
        origin_x: Millimeters(origin_x),
        origin_y: Millimeters(origin_y),
    })
}

/// Automatically calculates maximum number of rows and columns that fit onto the sheet.
pub fn calculate_max_fit(
    sheet: DimensionsMm,
    photo: DimensionsMm,
    margins: MarginsMm,
    gutters: GuttersMm,
) -> (u32, u32) {
    if !sheet.is_valid() || !photo.is_valid() || !margins.is_valid() || !gutters.is_valid() {
        return (1, 1);
    }

    let avail_w = sheet.width.0 - (margins.left.0 + margins.right.0);
    let avail_h = sheet.height.0 - (margins.top.0 + margins.bottom.0);

    if avail_w < photo.width.0 || avail_h < photo.height.0 {
        return (1, 1);
    }

    // avail_w >= cols * photo_w + (cols - 1) * gutter_w
    // avail_w + gutter_w >= cols * (photo_w + gutter_w)
    // cols = (avail_w + gutter_w) / (photo_w + gutter_w)
    let cols =
        ((avail_w + gutters.horizontal.0) / (photo.width.0 + gutters.horizontal.0)).floor() as u32;
    let rows =
        ((avail_h + gutters.vertical.0) / (photo.height.0 + gutters.vertical.0)).floor() as u32;

    (rows.max(1), cols.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_us_passport_on_4x6_sheet() {
        // 4x6 in = 101.6 x 152.4 mm
        // US passport: 50.8 x 50.8 mm (2x2 in)
        // 2 cols * 50.8 = 101.6 mm. Fits exactly 2 columns and 2 rows!
        let config = GridConfig {
            sheet: DimensionsMm::new(101.6, 152.4),
            photo: DimensionsMm::new(50.8, 50.8),
            rows: 2,
            cols: 2,
            margins: MarginsMm::ZERO,
            gutters: GuttersMm::ZERO,
            auto_center: true,
        };

        let result = compute_layout(&config);
        assert!(result.is_ok());
        let geom = result.unwrap();
        assert_eq!(geom.slots.len(), 4);
        assert_eq!(geom.grid_width, Millimeters(101.6));
        assert_eq!(geom.grid_height, Millimeters(101.6));
        // Centered vertically on 152.4 mm sheet: (152.4 - 101.6) / 2 = 25.4 mm
        assert!((geom.origin_y.0 - 25.4).abs() < 1e-4);
    }

    #[test]
    fn test_overflow_detection() {
        // 3 cols of 50.8 mm = 152.4 mm, but sheet width is only 101.6 mm
        let config = GridConfig {
            sheet: DimensionsMm::new(101.6, 152.4),
            photo: DimensionsMm::new(50.8, 50.8),
            rows: 2,
            cols: 3,
            margins: MarginsMm::ZERO,
            gutters: GuttersMm::ZERO,
            auto_center: true,
        };

        let result = compute_layout(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            LayoutError::OverflowWidth {
                required,
                available,
            } => {
                assert!((required.0 - 152.4).abs() < 1e-4);
                assert!((available.0 - 101.6).abs() < 1e-4);
            }
            other => panic!("Unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_zero_dimensions_error() {
        let config = GridConfig {
            sheet: DimensionsMm::new(0.0, 100.0),
            photo: DimensionsMm::new(50.0, 50.0),
            rows: 1,
            cols: 1,
            margins: MarginsMm::ZERO,
            gutters: GuttersMm::ZERO,
            auto_center: false,
        };
        assert!(compute_layout(&config).is_err());

        let config_photo_zero = GridConfig {
            sheet: DimensionsMm::new(100.0, 100.0),
            photo: DimensionsMm::new(50.0, -10.0),
            rows: 1,
            cols: 1,
            margins: MarginsMm::ZERO,
            gutters: GuttersMm::ZERO,
            auto_center: false,
        };
        assert!(compute_layout(&config_photo_zero).is_err());
    }

    #[test]
    fn test_zero_rows_or_cols_error() {
        let config_zero_rows = GridConfig {
            rows: 0,
            ..Default::default()
        };
        assert!(matches!(
            compute_layout(&config_zero_rows),
            Err(LayoutError::ZeroGridCount { .. })
        ));

        let config_zero_cols = GridConfig {
            rows: 1,
            cols: 0,
            ..Default::default()
        };
        assert!(matches!(
            compute_layout(&config_zero_cols),
            Err(LayoutError::ZeroGridCount { .. })
        ));
    }

    #[test]
    fn test_margins_exceeding_sheet_error() {
        let config = GridConfig {
            sheet: DimensionsMm::new(100.0, 100.0),
            photo: DimensionsMm::new(10.0, 10.0),
            rows: 1,
            cols: 1,
            margins: MarginsMm::new(60.0, 50.0, 10.0, 10.0),
            gutters: GuttersMm::ZERO,
            auto_center: false,
        };
        assert!(matches!(
            compute_layout(&config),
            Err(LayoutError::MarginsExceedSheetWidth { .. })
        ));
    }

    #[test]
    fn test_calculate_max_fit() {
        // Sheet: 100 x 150 mm
        // Photo: 30 x 40 mm
        // Gutters: 5 mm
        // Margins: 5 mm all around
        // Printable: 90 x 140 mm
        // Cols: (90 + 5) / (30 + 5) = 95 / 35 = 2.71 -> 2 cols
        // Rows: (140 + 5) / (40 + 5) = 145 / 45 = 3.22 -> 3 rows
        let (rows, cols) = calculate_max_fit(
            DimensionsMm::new(100.0, 150.0),
            DimensionsMm::new(30.0, 40.0),
            MarginsMm::uniform(5.0),
            GuttersMm::uniform(5.0),
        );
        assert_eq!(rows, 3);
        assert_eq!(cols, 2);
    }

    #[test]
    fn test_unusual_aspect_ratios_and_orientation() {
        // Panoramic photo 80 x 20 mm on 100 x 100 mm sheet
        let config = GridConfig {
            sheet: DimensionsMm::new(100.0, 100.0),
            photo: DimensionsMm::new(80.0, 20.0),
            rows: 4,
            cols: 1,
            margins: MarginsMm::ZERO,
            gutters: GuttersMm::new(0.0, 5.0),
            auto_center: true,
        };
        // 4 * 20 + 3 * 5 = 80 + 15 = 95 mm height. Fits!
        let result = compute_layout(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_photo_slot_pixel_rounding() {
        let slot = PhotoSlotMm {
            x: Millimeters(25.4),
            y: Millimeters(50.8),
            width: Millimeters(25.4),
            height: Millimeters(50.8),
            row: 0,
            col: 0,
        };
        let (px_x, px_y, px_w, px_h) = slot.to_pixel_rect(Dpi::STANDARD);
        assert_eq!(px_x, Pixels(300));
        assert_eq!(px_y, Pixels(600));
        assert_eq!(px_w, Pixels(300));
        assert_eq!(px_h, Pixels(600));
    }
}
