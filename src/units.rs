//! Physical unit types, conversions, and rounding policies.
//!
//! Imprint bases all print layout geometry on physical measurements (millimeters, inches)
//! converted to target output pixels at a specified DPI.

use std::fmt;

/// 1 inch in millimeters.
pub const MM_PER_INCH: f32 = 25.4;

/// Physical dimension in millimeters.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Millimeters(pub f32);

impl Millimeters {
    pub const ZERO: Self = Self(0.0);

    #[inline]
    pub fn new(val: f32) -> Self {
        Self(val)
    }

    #[inline]
    pub fn value(self) -> f32 {
        self.0
    }

    #[inline]
    pub fn to_inches(self) -> Inches {
        Inches(self.0 / MM_PER_INCH)
    }

    /// Converts physical millimeters to discrete pixels at a given DPI,
    /// applying standard half-up nearest pixel rounding to avoid layout drift.
    #[inline]
    pub fn to_pixels(self, dpi: Dpi) -> Pixels {
        let px = (self.0 / MM_PER_INCH) * (dpi.0 as f32);
        Pixels(px.round().max(0.0) as u32)
    }

    /// Validates that dimension is positive and finite.
    pub fn is_valid(self) -> bool {
        self.0.is_finite() && self.0 > 0.0
    }
}

impl fmt::Display for Millimeters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1} mm", self.0)
    }
}

/// Physical dimension in inches.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Inches(pub f32);

impl Inches {
    pub const ZERO: Self = Self(0.0);

    #[inline]
    pub fn new(val: f32) -> Self {
        Self(val)
    }

    #[inline]
    pub fn value(self) -> f32 {
        self.0
    }

    #[inline]
    pub fn to_mm(self) -> Millimeters {
        Millimeters(self.0 * MM_PER_INCH)
    }

    #[inline]
    pub fn to_pixels(self, dpi: Dpi) -> Pixels {
        self.to_mm().to_pixels(dpi)
    }

    pub fn is_valid(self) -> bool {
        self.0.is_finite() && self.0 > 0.0
    }
}

impl fmt::Display for Inches {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2} in", self.0)
    }
}

/// Resolution in dots per inch (DPI/PPI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dpi(pub u32);

impl Dpi {
    pub const DRAFT: Self = Self(150);
    pub const STANDARD: Self = Self(300);
    pub const HIGH: Self = Self(600);

    #[inline]
    pub fn new(val: u32) -> Self {
        Self(val)
    }

    #[inline]
    pub fn value(self) -> u32 {
        self.0
    }

    pub fn is_valid(self) -> bool {
        self.0 > 0
    }
}

impl fmt::Display for Dpi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} DPI", self.0)
    }
}

/// Discrete pixel dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pixels(pub u32);

impl Pixels {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub fn new(val: u32) -> Self {
        Self(val)
    }

    #[inline]
    pub fn value(self) -> u32 {
        self.0
    }

    /// Converts discrete pixels back to physical millimeters at the given DPI.
    #[inline]
    pub fn to_mm(self, dpi: Dpi) -> Millimeters {
        if dpi.0 == 0 {
            Millimeters(0.0)
        } else {
            Millimeters((self.0 as f32 / dpi.0 as f32) * MM_PER_INCH)
        }
    }
}

impl fmt::Display for Pixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} px", self.0)
    }
}

/// 2D physical dimensions in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionsMm {
    pub width: Millimeters,
    pub height: Millimeters,
}

impl DimensionsMm {
    pub const ZERO: Self = Self {
        width: Millimeters::ZERO,
        height: Millimeters::ZERO,
    };

    #[inline]
    pub fn new(width_mm: f32, height_mm: f32) -> Self {
        Self {
            width: Millimeters(width_mm),
            height: Millimeters(height_mm),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.width.is_valid() && self.height.is_valid()
    }

    pub fn aspect_ratio(&self) -> f32 {
        if self.height.0 > 0.0 {
            self.width.0 / self.height.0
        } else {
            0.0
        }
    }

    pub fn to_pixels(&self, dpi: Dpi) -> (Pixels, Pixels) {
        (self.width.to_pixels(dpi), self.height.to_pixels(dpi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mm_inch_conversions() {
        let mm = Millimeters(25.4);
        let inch = mm.to_inches();
        assert!((inch.0 - 1.0).abs() < 1e-6);

        let roundtrip = inch.to_mm();
        assert!((roundtrip.0 - 25.4).abs() < 1e-6);

        // 2x2 inches (US passport)
        let us_pass_inch = Inches(2.0);
        let us_pass_mm = us_pass_inch.to_mm();
        assert!((us_pass_mm.0 - 50.8).abs() < 1e-6);
    }

    #[test]
    fn test_mm_to_pixels_at_standard_dpi() {
        // At 300 DPI: 1 inch (25.4 mm) should be exactly 300 pixels
        let one_inch_mm = Millimeters(25.4);
        assert_eq!(one_inch_mm.to_pixels(Dpi::STANDARD), Pixels(300));

        // 2 inches (50.8 mm) at 300 DPI should be 600 pixels
        let two_inches_mm = Millimeters(50.8);
        assert_eq!(two_inches_mm.to_pixels(Dpi::STANDARD), Pixels(600));

        // 35 x 45 mm (EU / Schengen standard) at 300 DPI
        // 35 / 25.4 * 300 = 413.3858 -> 413 px
        // 45 / 25.4 * 300 = 531.496 -> 531 px
        let eu_w = Millimeters(35.0);
        let eu_h = Millimeters(45.0);
        assert_eq!(eu_w.to_pixels(Dpi::STANDARD), Pixels(413));
        assert_eq!(eu_h.to_pixels(Dpi::STANDARD), Pixels(531));
    }

    #[test]
    fn test_pixel_to_mm_roundtrip() {
        let dpi = Dpi(300);
        let px = Pixels(300);
        let mm = px.to_mm(dpi);
        assert!((mm.0 - 25.4).abs() < 1e-4);
    }

    #[test]
    fn test_validity_checks() {
        assert!(Millimeters(10.0).is_valid());
        assert!(!Millimeters(0.0).is_valid());
        assert!(!Millimeters(-5.0).is_valid());
        assert!(!Millimeters(f32::NAN).is_valid());
        assert!(!Millimeters(f32::INFINITY).is_valid());

        assert!(Dpi(300).is_valid());
        assert!(!Dpi(0).is_valid());
    }

    #[test]
    fn test_aspect_ratio() {
        let dims = DimensionsMm::new(35.0, 45.0);
        assert!((dims.aspect_ratio() - (35.0 / 45.0)).abs() < 1e-6);

        let square = DimensionsMm::new(50.8, 50.8);
        assert!((square.aspect_ratio() - 1.0).abs() < 1e-6);

        let invalid = DimensionsMm::new(50.0, 0.0);
        assert_eq!(invalid.aspect_ratio(), 0.0);
    }
}
