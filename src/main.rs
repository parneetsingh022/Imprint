//! Imprint desktop application entrypoint.

use eframe::egui;
use imprint::ImprintApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Imprint — Passport & ID Photo Print Studio")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Imprint",
        options,
        Box::new(|cc| Ok(Box::new(ImprintApp::new(cc)))),
    )
}
