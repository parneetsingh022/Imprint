//! Standard presets for passport photo sizes and printable paper sheets.

use crate::units::{DimensionsMm, Millimeters};

/// Predefined standard passport and ID photo sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoPreset {
    UsPassport,
    EuSchengen,
    UkPassport,
    CanadianPassport,
    IndianPassport,
    ChinesePassport,
    JapanesePassport,
    Custom,
}

impl PhotoPreset {
    pub const ALL: &'static [PhotoPreset] = &[
        PhotoPreset::UsPassport,
        PhotoPreset::EuSchengen,
        PhotoPreset::UkPassport,
        PhotoPreset::CanadianPassport,
        PhotoPreset::IndianPassport,
        PhotoPreset::ChinesePassport,
        PhotoPreset::JapanesePassport,
        PhotoPreset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PhotoPreset::UsPassport => "US / India / OCI (2 × 2 in / 50.8 × 50.8 mm)",
            PhotoPreset::EuSchengen => "EU / Schengen (35 × 45 mm)",
            PhotoPreset::UkPassport => "UK Passport (35 × 45 mm)",
            PhotoPreset::CanadianPassport => "Canada (50 × 70 mm)",
            PhotoPreset::IndianPassport => "India Visa / Passport (51 × 51 mm)",
            PhotoPreset::ChinesePassport => "China (33 × 48 mm)",
            PhotoPreset::JapanesePassport => "Japan (35 × 45 mm)",
            PhotoPreset::Custom => "Custom Size",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            PhotoPreset::UsPassport => "US (2×2 in)",
            PhotoPreset::EuSchengen => "EU/Schengen (35×45mm)",
            PhotoPreset::UkPassport => "UK (35×45mm)",
            PhotoPreset::CanadianPassport => "Canada (50×70mm)",
            PhotoPreset::IndianPassport => "India (51×51mm)",
            PhotoPreset::ChinesePassport => "China (33×48mm)",
            PhotoPreset::JapanesePassport => "Japan (35×45mm)",
            PhotoPreset::Custom => "Custom",
        }
    }

    /// Returns physical dimensions in millimeters, or `None` if custom.
    pub fn dimensions(self) -> Option<DimensionsMm> {
        match self {
            PhotoPreset::UsPassport => Some(DimensionsMm::new(50.8, 50.8)),
            PhotoPreset::EuSchengen | PhotoPreset::UkPassport | PhotoPreset::JapanesePassport => {
                Some(DimensionsMm::new(35.0, 45.0))
            }
            PhotoPreset::CanadianPassport => Some(DimensionsMm::new(50.0, 70.0)),
            PhotoPreset::IndianPassport => Some(DimensionsMm::new(51.0, 51.0)),
            PhotoPreset::ChinesePassport => Some(DimensionsMm::new(33.0, 48.0)),
            PhotoPreset::Custom => None,
        }
    }
}

/// Paper sheet orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetOrientation {
    Portrait,
    Landscape,
}

impl SheetOrientation {
    pub fn toggle(self) -> Self {
        match self {
            SheetOrientation::Portrait => SheetOrientation::Landscape,
            SheetOrientation::Landscape => SheetOrientation::Portrait,
        }
    }
}

/// Predefined sheet sizes for printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetPreset {
    Photo4x6,
    Photo5x7,
    A4,
    Letter,
    Custom,
}

impl SheetPreset {
    pub const ALL: &'static [SheetPreset] = &[
        SheetPreset::Photo4x6,
        SheetPreset::Photo5x7,
        SheetPreset::A4,
        SheetPreset::Letter,
        SheetPreset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SheetPreset::Photo4x6 => "4 × 6 in (101.6 × 152.4 mm / 10×15 cm)",
            SheetPreset::Photo5x7 => "5 × 7 in (127 × 177.8 mm / 13×18 cm)",
            SheetPreset::A4 => "A4 (210 × 297 mm)",
            SheetPreset::Letter => "US Letter (8.5 × 11 in / 215.9 × 279.4 mm)",
            SheetPreset::Custom => "Custom Sheet",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            SheetPreset::Photo4x6 => "4×6\"",
            SheetPreset::Photo5x7 => "5×7\"",
            SheetPreset::A4 => "A4",
            SheetPreset::Letter => "Letter",
            SheetPreset::Custom => "Custom",
        }
    }

    /// Base dimensions in millimeters in portrait orientation (width <= height).
    pub fn base_dimensions(self) -> Option<DimensionsMm> {
        match self {
            SheetPreset::Photo4x6 => Some(DimensionsMm::new(101.6, 152.4)),
            SheetPreset::Photo5x7 => Some(DimensionsMm::new(127.0, 177.8)),
            SheetPreset::A4 => Some(DimensionsMm::new(210.0, 297.0)),
            SheetPreset::Letter => Some(DimensionsMm::new(215.9, 279.4)),
            SheetPreset::Custom => None,
        }
    }

    /// Dimensions taking orientation into account.
    pub fn oriented_dimensions(self, orientation: SheetOrientation) -> Option<DimensionsMm> {
        let base = self.base_dimensions()?;
        match orientation {
            SheetOrientation::Portrait => Some(base),
            SheetOrientation::Landscape => Some(DimensionsMm {
                width: Millimeters(base.height.0),
                height: Millimeters(base.width.0),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_preset_dimensions() {
        let us = PhotoPreset::UsPassport.dimensions().expect("should exist");
        assert_eq!(us.width, Millimeters(50.8));
        assert_eq!(us.height, Millimeters(50.8));

        let eu = PhotoPreset::EuSchengen.dimensions().expect("should exist");
        assert_eq!(eu.width, Millimeters(35.0));
        assert_eq!(eu.height, Millimeters(45.0));

        assert!(PhotoPreset::Custom.dimensions().is_none());
    }

    #[test]
    fn test_sheet_orientation_toggle() {
        let p = SheetOrientation::Portrait;
        let l = p.toggle();
        assert_eq!(l, SheetOrientation::Landscape);
        assert_eq!(l.toggle(), SheetOrientation::Portrait);
    }

    #[test]
    fn test_sheet_preset_orientation() {
        let base = SheetPreset::Photo4x6
            .oriented_dimensions(SheetOrientation::Portrait)
            .expect("should exist");
        assert_eq!(base.width, Millimeters(101.6));
        assert_eq!(base.height, Millimeters(152.4));

        let landscape = SheetPreset::Photo4x6
            .oriented_dimensions(SheetOrientation::Landscape)
            .expect("should exist");
        assert_eq!(landscape.width, Millimeters(152.4));
        assert_eq!(landscape.height, Millimeters(101.6));
    }
}
