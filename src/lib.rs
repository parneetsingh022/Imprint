//! Imprint: Passport and ID photo print layout engine and desktop application.
//!
//! Provides physical unit calculations, standard passport/sheet presets,
//! grid arrangement, sheet bounds validation, and an interactive egui GUI.

pub mod app;
pub mod export;
pub mod image_ops;
pub mod layout;
pub mod preset;
pub mod ui_icons;
pub mod units;

pub use app::ImprintApp;
