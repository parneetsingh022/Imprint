//! Main GUI application state, interactive canvas, crop tool, and print export.

use std::path::Path;

use eframe::egui::{self, Color32, Panel, Pos2, Rect, Stroke, Ui, Vec2};

use crate::export::{ExportFormat, ExportOptions, ExportTarget, export_to_file};
use crate::image_ops::{CropArea, CropCorner, LoadedImage, crop_and_resample, to_egui_color_image};
use crate::layout::{
    GridConfig, GuttersMm, LayoutGeometry, MarginsMm, calculate_max_fit, compute_layout,
};
use crate::preset::{PhotoPreset, SheetOrientation, SheetPreset};
use crate::units::{DimensionsMm, Dpi, Millimeters};

/// Active tool mode selected in the tool sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    SelectPan,
    Crop,
    Grid,
    Guides,
}

impl ToolMode {
    pub fn name(self) -> &'static str {
        match self {
            ToolMode::SelectPan => "Pan / Select",
            ToolMode::Crop => "Crop Photo",
            ToolMode::Grid => "Grid Layout",
            ToolMode::Guides => "Guides & Marks",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ToolMode::SelectPan => "✋",
            ToolMode::Crop => "✂",
            ToolMode::Grid => "▦",
            ToolMode::Guides => "📏",
        }
    }
}

/// Active drag operation within the crop editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CropDragMode {
    #[default]
    None,
    Move,
    Corner(CropCorner),
}

/// The main Imprint application state.
pub struct ImprintApp {
    // Presets & orientation
    pub photo_preset: PhotoPreset,
    pub sheet_preset: SheetPreset,
    pub sheet_orientation: SheetOrientation,
    pub dpi: Dpi,

    // Layout configuration
    pub grid_config: GridConfig,

    // Loaded photograph & crop
    pub loaded_image: Option<LoadedImage>,
    pub crop_area: CropArea,
    pub source_texture: Option<egui::TextureHandle>,
    pub cropped_texture: Option<egui::TextureHandle>,
    pub texture_needs_update: bool,
    pub crop_drag_mode: CropDragMode,

    // Export options
    pub export_target: ExportTarget,
    pub export_format: ExportFormat,

    // Interaction & View
    pub active_tool: ToolMode,
    pub canvas_zoom: f32,
    pub canvas_pan: Vec2,
    pub selected_slot: Option<usize>,

    // View Options
    pub show_margins: bool,
    pub show_cut_marks: bool,
    pub show_dimensions: bool,
    pub show_biometric_guides: bool,

    // Status and dialogs
    pub status_message: String,
    pub show_about_dialog: bool,
}

impl Default for ImprintApp {
    fn default() -> Self {
        let config = GridConfig {
            sheet: SheetPreset::Photo4x6
                .oriented_dimensions(SheetOrientation::Portrait)
                .unwrap_or(DimensionsMm::new(101.6, 152.4)),
            photo: PhotoPreset::UsPassport
                .dimensions()
                .unwrap_or(DimensionsMm::new(50.8, 50.8)),
            rows: 2,
            cols: 2,
            margins: MarginsMm::new(2.0, 2.0, 2.0, 2.0),
            gutters: GuttersMm::new(2.0, 2.0),
            auto_center: true,
        };

        Self {
            photo_preset: PhotoPreset::UsPassport,
            sheet_preset: SheetPreset::Photo4x6,
            sheet_orientation: SheetOrientation::Portrait,
            dpi: Dpi::STANDARD,
            grid_config: config,
            loaded_image: None,
            crop_area: CropArea::default(),
            source_texture: None,
            cropped_texture: None,
            texture_needs_update: false,
            crop_drag_mode: CropDragMode::None,
            export_target: ExportTarget::FullSheet,
            export_format: ExportFormat::Pdf,
            active_tool: ToolMode::SelectPan,
            canvas_zoom: 1.0,
            canvas_pan: Vec2::ZERO,
            selected_slot: None,
            show_margins: true,
            show_cut_marks: true,
            show_dimensions: true,
            show_biometric_guides: true,
            status_message: "Ready. Import a photo or configure layout to begin.".to_string(),
            show_about_dialog: false,
        }
    }
}

impl ImprintApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Updates sheet dimensions according to current preset and orientation.
    pub fn apply_sheet_preset(&mut self) {
        if let Some(dims) = self
            .sheet_preset
            .oriented_dimensions(self.sheet_orientation)
        {
            self.grid_config.sheet = dims;
        }
    }

    /// Updates photo dimensions according to current photo preset.
    pub fn apply_photo_preset(&mut self) {
        if let Some(dims) = self.photo_preset.dimensions() {
            self.grid_config.photo = dims;
            if let Some(loaded) = &self.loaded_image {
                self.crop_area
                    .adjust_aspect(loaded.width, loaded.height, dims.aspect_ratio());
                self.texture_needs_update = true;
            }
        }
    }

    /// Automatically fits maximum rows and columns for current parameters.
    pub fn auto_fit_grid(&mut self) {
        let (rows, cols) = calculate_max_fit(
            self.grid_config.sheet,
            self.grid_config.photo,
            self.grid_config.margins,
            self.grid_config.gutters,
        );
        self.grid_config.rows = rows;
        self.grid_config.cols = cols;
        self.status_message = format!("Auto-fitted grid to {rows} rows × {cols} columns.");
    }

    /// Resets canvas pan and zoom.
    pub fn reset_view(&mut self) {
        self.canvas_zoom = 1.0;
        self.canvas_pan = Vec2::ZERO;
        self.status_message = "Canvas view reset to 100%.".to_string();
    }

    /// Loads an image file into the application and configures default crop.
    pub fn load_image_from_path(&mut self, path: &Path) {
        match LoadedImage::from_path(path) {
            Ok(loaded) => {
                let target_aspect = self.grid_config.photo.aspect_ratio();
                self.crop_area =
                    CropArea::default_for_aspect(loaded.width, loaded.height, target_aspect);
                self.status_message = format!(
                    "Loaded image: {} ({} × {} px)",
                    loaded.file_name, loaded.width, loaded.height
                );
                self.loaded_image = Some(loaded);
                self.source_texture = None;
                self.cropped_texture = None;
                self.texture_needs_update = true;
                self.active_tool = ToolMode::Crop;
            }
            Err(err) => {
                self.status_message = format!("Failed to open image {}: {err}", path.display());
            }
        }
    }

    /// Opens native file dialog to import an image.
    pub fn import_image_dialog(&mut self) {
        let dialog = rfd::FileDialog::new()
            .add_filter(
                "Image Files (*.png, *.jpg, *.jpeg, *.webp, *.bmp)",
                &["png", "jpg", "jpeg", "webp", "bmp"],
            )
            .set_title("Select Portrait Photograph");

        if let Some(path) = dialog.pick_file() {
            self.load_image_from_path(&path);
        }
    }

    /// Opens native file save dialog to export to PDF or image.
    pub fn export_dialog(&mut self, target: ExportTarget) {
        let loaded = match &self.loaded_image {
            Some(img) => img,
            None => {
                self.status_message =
                    "No photo loaded. Please import an image before exporting.".to_string();
                return;
            }
        };

        let ext = self.export_format.extension();
        let filter_label = self.export_format.label();

        let default_name = match target {
            ExportTarget::FullSheet => format!(
                "imprint_sheet_{}_{}dpi.{}",
                self.sheet_preset.short_name().replace(['"', '/', ' '], "_"),
                self.dpi.0,
                ext
            ),
            ExportTarget::SinglePhoto => format!(
                "imprint_photo_{}_{}dpi.{}",
                self.photo_preset.short_name().replace(['"', '/', ' '], "_"),
                self.dpi.0,
                ext
            ),
        };

        let dialog = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(filter_label, &[ext])
            .set_title(format!("Export {}", target.label()));

        if let Some(dest_path) = dialog.save_file() {
            let options = ExportOptions::new(
                self.dpi,
                target,
                Some(self.export_format),
                self.show_cut_marks,
            );
            match export_to_file(
                &dest_path,
                &self.grid_config,
                &loaded.original,
                &self.crop_area,
                options,
            ) {
                Ok(report) => {
                    self.status_message = format!(
                        "✓ Exported {} to {} ({}×{} px @ {}, {:.1} KB)",
                        report.target.label(),
                        report
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("file"),
                        report.width_px,
                        report.height_px,
                        report.dpi,
                        report.file_size_bytes as f64 / 1024.0,
                    );
                }
                Err(err) => {
                    self.status_message = format!("⚠ Export failed: {err}");
                }
            }
        } else {
            self.status_message = "Export cancelled.".to_string();
        }
    }

    /// Zooms the crop box in or out around its center.
    pub fn zoom_crop(&mut self, factor: f32) {
        let center_x = self.crop_area.x + self.crop_area.width / 2.0;
        let center_y = self.crop_area.y + self.crop_area.height / 2.0;

        let new_w = (self.crop_area.width * factor).clamp(0.05, 1.0);
        let new_h = (self.crop_area.height * factor).clamp(0.05, 1.0);

        self.crop_area.width = new_w;
        self.crop_area.height = new_h;
        self.crop_area.x = (center_x - new_w / 2.0).clamp(0.0, (1.0 - new_w).max(0.0));
        self.crop_area.y = (center_y - new_h / 2.0).clamp(0.0, (1.0 - new_h).max(0.0));
        self.crop_area.clamp_within_bounds();
        self.texture_needs_update = true;
    }

    /// Updates textures for egui GPU rendering when the crop area or image changes.
    pub fn update_textures_if_needed(&mut self, ctx: &egui::Context) {
        let Some(loaded) = &self.loaded_image else {
            self.source_texture = None;
            self.cropped_texture = None;
            return;
        };

        // Source texture for crop tool (uploaded once, downsampled if extremely large for fast GPU display)
        if self.source_texture.is_none() {
            let max_dim = 2560;
            let (w, h) = (loaded.width, loaded.height);
            let display_rgba = if w > max_dim || h > max_dim {
                let (dw, dh) = if w >= h {
                    (
                        max_dim,
                        ((h as f32 / w as f32) * max_dim as f32).round() as u32,
                    )
                } else {
                    (
                        ((w as f32 / h as f32) * max_dim as f32).round() as u32,
                        max_dim,
                    )
                };
                ::image::imageops::resize(
                    &loaded.original.to_rgba8(),
                    dw,
                    dh,
                    ::image::imageops::FilterType::Triangle,
                )
            } else {
                loaded.original.to_rgba8()
            };
            let color_img = to_egui_color_image(&display_rgba);
            self.source_texture =
                Some(ctx.load_texture("source_photo", color_img, egui::TextureOptions::LINEAR));
        }

        // PERFORMANCE: While in the crop tool, skip expensive CPU resampling!
        // The crop tool renders the source texture with real-time vector overlay guides.
        // We regenerate cropped_texture only when displaying the sheet layout.
        if self.active_tool == ToolMode::Crop {
            return;
        }

        if !self.texture_needs_update {
            return;
        }
        self.texture_needs_update = false;

        let aspect = self.grid_config.photo.aspect_ratio();
        let (prev_w, prev_h) = if aspect >= 1.0 {
            (512, (512.0 / aspect).round().max(16.0) as u32)
        } else {
            ((512.0 * aspect).round().max(16.0) as u32, 512)
        };

        if let Ok(cropped_rgba) =
            crop_and_resample(&loaded.original, &self.crop_area, prev_w, prev_h)
        {
            let color_img = to_egui_color_image(&cropped_rgba);
            self.cropped_texture = Some(ctx.load_texture(
                "cropped_slot_photo",
                color_img,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    /// Handles files dragged and dropped onto the window.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.into_iter().next() {
            let path = file.path();
            self.load_image_from_path(path);
        }
    }
}

impl eframe::App for ImprintApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ui.ctx());
        self.update_textures_if_needed(ui.ctx());

        self.render_menu_bar(ui);
        self.render_status_bar(ui);
        self.render_tool_sidebar(ui);
        self.render_properties_panel(ui);
        self.render_central_canvas(ui);
        self.render_dialogs(ui);
    }
}

impl ImprintApp {
    /// Renders the top menu bar.
    fn render_menu_bar(&mut self, ui: &mut Ui) {
        Panel::top("top_menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Import Image...").clicked() {
                        self.import_image_dialog();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Export Print Layout (Full Sheet)...").clicked() {
                        self.export_dialog(ExportTarget::FullSheet);
                        ui.close();
                    }
                    if ui.button("Export Single Photo...").clicked() {
                        self.export_dialog(ExportTarget::SinglePhoto);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Auto-Fit Grid to Sheet").clicked() {
                        self.auto_fit_grid();
                        ui.close();
                    }
                    if ui.button("Reset View (100%)").clicked() {
                        self.reset_view();
                        ui.close();
                    }
                    if ui.button("Reset Layout to Defaults").clicked() {
                        *self = Self::default();
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Zoom In (+10%)").clicked() {
                        self.canvas_zoom = (self.canvas_zoom * 1.1).min(5.0);
                        ui.close();
                    }
                    if ui.button("Zoom Out (-10%)").clicked() {
                        self.canvas_zoom = (self.canvas_zoom / 1.1).max(0.2);
                        ui.close();
                    }
                    if ui.button("Reset Zoom (100%)").clicked() {
                        self.canvas_zoom = 1.0;
                        ui.close();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_margins, "Show Sheet Margins");
                    ui.checkbox(&mut self.show_cut_marks, "Show Cut Marks");
                    ui.checkbox(&mut self.show_dimensions, "Show Dimensions");
                    ui.checkbox(&mut self.show_biometric_guides, "Show Biometric Guides");
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About Imprint").clicked() {
                        self.show_about_dialog = true;
                        ui.close();
                    }
                });
            });
        });
    }

    /// Renders the left tool sidebar.
    fn render_tool_sidebar(&mut self, ui: &mut Ui) {
        Panel::left("tool_sidebar")
            .resizable(false)
            .exact_size(70.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("Tools");
                    ui.add_space(8.0);

                    let tools = [
                        ToolMode::SelectPan,
                        ToolMode::Crop,
                        ToolMode::Grid,
                        ToolMode::Guides,
                    ];

                    for tool in tools {
                        let selected = self.active_tool == tool;
                        let text = format!("{}\n{}", tool.icon(), tool.name());
                        if ui
                            .selectable_label(selected, egui::RichText::new(text).size(11.0))
                            .on_hover_text(tool.name())
                            .clicked()
                        {
                            self.active_tool = tool;
                            self.status_message = format!("Active tool: {}", tool.name());
                        }
                        ui.add_space(4.0);
                    }

                    ui.separator();
                    ui.add_space(4.0);

                    // Import shortcut button
                    if ui
                        .button("📁\nImport")
                        .on_hover_text("Import Image File")
                        .clicked()
                    {
                        self.import_image_dialog();
                    }

                    ui.add_space(4.0);

                    // Export shortcut button
                    if ui
                        .button("💾\nExport")
                        .on_hover_text("Export to PDF / Image")
                        .clicked()
                    {
                        self.export_dialog(self.export_target);
                    }

                    ui.separator();
                    ui.add_space(4.0);

                    // View navigation buttons
                    if ui.button("🔍+").on_hover_text("Zoom In").clicked() {
                        self.canvas_zoom = (self.canvas_zoom * 1.15).min(5.0);
                    }
                    if ui.button("🔍-").on_hover_text("Zoom Out").clicked() {
                        self.canvas_zoom = (self.canvas_zoom / 1.15).max(0.2);
                    }
                    if ui.button("1:1").on_hover_text("Reset Zoom").clicked() {
                        self.reset_view();
                    }
                    if ui.button("⚡ Fit").on_hover_text("Auto Fit Grid").clicked() {
                        self.auto_fit_grid();
                    }
                });
            });
    }

    /// Renders the right properties panel.
    fn render_properties_panel(&mut self, ui: &mut Ui) {
        Panel::right("properties_panel")
            .resizable(true)
            .default_size(310.0)
            .min_size(270.0)
            .max_size(460.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.heading("Properties & Settings");
                    ui.add_space(8.0);

                    // --- Active Photo & Crop ---
                    ui.group(|ui| {
                        ui.strong("📷 Active Photo & Crop");
                        ui.add_space(4.0);

                        if let Some(loaded) = &self.loaded_image {
                            ui.label(format!("File: {}", loaded.file_name));
                            ui.label(format!("Original: {} × {} px", loaded.width, loaded.height));
                            let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                            ui.label(format!(
                                "Target Print: {} × {} px @ {}",
                                px_w, px_h, self.dpi
                            ));

                            ui.horizontal(|ui| {
                                let is_cropping = self.active_tool == ToolMode::Crop;
                                if ui
                                    .selectable_label(is_cropping, "✂ Adjust Crop")
                                    .on_hover_text("Switch to interactive crop tool")
                                    .clicked()
                                {
                                    self.active_tool = ToolMode::Crop;
                                }

                                if ui
                                    .button("🔄 Replace...")
                                    .on_hover_text("Select a different photo file")
                                    .clicked()
                                {
                                    self.import_image_dialog();
                                }

                                if ui
                                    .button("🗑")
                                    .on_hover_text("Clear current photo")
                                    .clicked()
                                {
                                    self.loaded_image = None;
                                    self.source_texture = None;
                                    self.cropped_texture = None;
                                    self.status_message = "Photo removed.".to_string();
                                }
                            });
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(170, 175, 185),
                                "No photo imported.",
                            );
                            if ui
                                .button("📁 Import Portrait Photo...")
                                .on_hover_text("Choose a JPG, PNG, or WebP photo")
                                .clicked()
                            {
                                self.import_image_dialog();
                            }
                        }
                    });

                    ui.add_space(6.0);

                    // --- Export Section ---
                    ui.group(|ui| {
                        ui.strong("🖨 Export Output");
                        ui.add_space(4.0);

                        egui::ComboBox::from_label("Target")
                            .selected_text(self.export_target.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.export_target,
                                    ExportTarget::FullSheet,
                                    ExportTarget::FullSheet.label(),
                                );
                                ui.selectable_value(
                                    &mut self.export_target,
                                    ExportTarget::SinglePhoto,
                                    ExportTarget::SinglePhoto.label(),
                                );
                            });

                        egui::ComboBox::from_id_salt("export_format_combo")
                            .selected_text(self.export_format.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.export_format,
                                    ExportFormat::Pdf,
                                    ExportFormat::Pdf.label(),
                                );
                                ui.selectable_value(
                                    &mut self.export_format,
                                    ExportFormat::Png,
                                    ExportFormat::Png.label(),
                                );
                                ui.selectable_value(
                                    &mut self.export_format,
                                    ExportFormat::Jpeg,
                                    ExportFormat::Jpeg.label(),
                                );
                            });

                        let (out_w, out_h) = match self.export_target {
                            ExportTarget::FullSheet => self.grid_config.sheet.to_pixels(self.dpi),
                            ExportTarget::SinglePhoto => self.grid_config.photo.to_pixels(self.dpi),
                        };
                        ui.label(format!("Resolution: {} × {} px", out_w, out_h));

                        let has_photo = self.loaded_image.is_some();
                        let btn =
                            egui::Button::new(egui::RichText::new("💾 Export to File...").strong());
                        if ui.add_enabled(has_photo, btn).clicked() {
                            self.export_dialog(self.export_target);
                        }

                        if !has_photo {
                            ui.colored_label(
                                Color32::from_rgb(200, 160, 50),
                                "Import a photo to enable export",
                            );
                        }
                    });

                    ui.add_space(6.0);

                    // --- Sheet Setup ---
                    ui.group(|ui| {
                        ui.strong("📄 Sheet Setup");
                        ui.add_space(4.0);

                        let old_preset = self.sheet_preset;
                        egui::ComboBox::from_label("Sheet Preset")
                            .selected_text(self.sheet_preset.short_name())
                            .show_ui(ui, |ui| {
                                for preset in SheetPreset::ALL {
                                    ui.selectable_value(
                                        &mut self.sheet_preset,
                                        *preset,
                                        preset.label(),
                                    );
                                }
                            });
                        if self.sheet_preset != old_preset {
                            self.apply_sheet_preset();
                        }

                        ui.horizontal(|ui| {
                            ui.label("Orientation:");
                            let p_selected = self.sheet_orientation == SheetOrientation::Portrait;
                            if ui.selectable_label(p_selected, "Portrait").clicked() && !p_selected
                            {
                                self.sheet_orientation = SheetOrientation::Portrait;
                                self.apply_sheet_preset();
                            }
                            let l_selected = self.sheet_orientation == SheetOrientation::Landscape;
                            if ui.selectable_label(l_selected, "Landscape").clicked() && !l_selected
                            {
                                self.sheet_orientation = SheetOrientation::Landscape;
                                self.apply_sheet_preset();
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Width (mm):");
                            let mut w = self.grid_config.sheet.width.0;
                            if ui
                                .add(egui::DragValue::new(&mut w).speed(0.5).range(10.0..=1000.0))
                                .changed()
                            {
                                self.grid_config.sheet.width = Millimeters(w);
                                self.sheet_preset = SheetPreset::Custom;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Height (mm):");
                            let mut h = self.grid_config.sheet.height.0;
                            if ui
                                .add(egui::DragValue::new(&mut h).speed(0.5).range(10.0..=1000.0))
                                .changed()
                            {
                                self.grid_config.sheet.height = Millimeters(h);
                                self.sheet_preset = SheetPreset::Custom;
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // --- Photo Dimensions ---
                    ui.group(|ui| {
                        ui.strong("📷 Photo Dimensions");
                        ui.add_space(4.0);

                        let old_photo_preset = self.photo_preset;
                        egui::ComboBox::from_label("Passport Preset")
                            .selected_text(self.photo_preset.short_name())
                            .show_ui(ui, |ui| {
                                for preset in PhotoPreset::ALL {
                                    ui.selectable_value(
                                        &mut self.photo_preset,
                                        *preset,
                                        preset.label(),
                                    );
                                }
                            });
                        if self.photo_preset != old_photo_preset {
                            self.apply_photo_preset();
                        }

                        ui.horizontal(|ui| {
                            ui.label("Width (mm):");
                            let mut w = self.grid_config.photo.width.0;
                            if ui
                                .add(egui::DragValue::new(&mut w).speed(0.5).range(5.0..=500.0))
                                .changed()
                            {
                                self.grid_config.photo.width = Millimeters(w);
                                self.photo_preset = PhotoPreset::Custom;
                                if let Some(loaded) = &self.loaded_image {
                                    self.crop_area.adjust_aspect(
                                        loaded.width,
                                        loaded.height,
                                        self.grid_config.photo.aspect_ratio(),
                                    );
                                    self.texture_needs_update = true;
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Height (mm):");
                            let mut h = self.grid_config.photo.height.0;
                            if ui
                                .add(egui::DragValue::new(&mut h).speed(0.5).range(5.0..=500.0))
                                .changed()
                            {
                                self.grid_config.photo.height = Millimeters(h);
                                self.photo_preset = PhotoPreset::Custom;
                                if let Some(loaded) = &self.loaded_image {
                                    self.crop_area.adjust_aspect(
                                        loaded.width,
                                        loaded.height,
                                        self.grid_config.photo.aspect_ratio(),
                                    );
                                    self.texture_needs_update = true;
                                }
                            }
                        });

                        let aspect = self.grid_config.photo.aspect_ratio();
                        ui.label(format!("Aspect Ratio: {:.2} : 1", aspect));

                        ui.horizontal(|ui| {
                            ui.label("Print DPI:");
                            let old_dpi = self.dpi;
                            egui::ComboBox::from_id_salt("dpi_combobox")
                                .selected_text(format!("{}", self.dpi))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.dpi,
                                        Dpi::DRAFT,
                                        "150 DPI (Draft)",
                                    );
                                    ui.selectable_value(
                                        &mut self.dpi,
                                        Dpi::STANDARD,
                                        "300 DPI (Standard Print)",
                                    );
                                    ui.selectable_value(
                                        &mut self.dpi,
                                        Dpi::HIGH,
                                        "600 DPI (High Resolution)",
                                    );
                                });
                            if self.dpi != old_dpi {
                                self.texture_needs_update = true;
                            }
                        });

                        let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                        ui.label(format!("Photo Pixels: {} × {}", px_w, px_h));
                    });

                    ui.add_space(6.0);

                    // --- Grid Layout ---
                    ui.group(|ui| {
                        ui.strong("▦ Grid Arrangement");
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Columns:");
                            ui.add(egui::DragValue::new(&mut self.grid_config.cols).range(1..=20));
                            ui.label("Rows:");
                            ui.add(egui::DragValue::new(&mut self.grid_config.rows).range(1..=20));
                        });

                        ui.horizontal(|ui| {
                            if ui.button("⚡ Max Fit").clicked() {
                                self.auto_fit_grid();
                            }
                            ui.checkbox(&mut self.grid_config.auto_center, "Auto Center");
                        });
                    });

                    ui.add_space(6.0);

                    // --- Margins & Spacing ---
                    ui.group(|ui| {
                        ui.strong("📐 Margins & Spacing (mm)");
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Left:");
                            let mut m = self.grid_config.margins.left.0;
                            if ui
                                .add(egui::DragValue::new(&mut m).speed(0.5).range(0.0..=100.0))
                                .changed()
                            {
                                self.grid_config.margins.left = Millimeters(m);
                            }
                            ui.label("Right:");
                            let mut m = self.grid_config.margins.right.0;
                            if ui
                                .add(egui::DragValue::new(&mut m).speed(0.5).range(0.0..=100.0))
                                .changed()
                            {
                                self.grid_config.margins.right = Millimeters(m);
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Top:");
                            let mut m = self.grid_config.margins.top.0;
                            if ui
                                .add(egui::DragValue::new(&mut m).speed(0.5).range(0.0..=100.0))
                                .changed()
                            {
                                self.grid_config.margins.top = Millimeters(m);
                            }
                            ui.label("Bottom:");
                            let mut m = self.grid_config.margins.bottom.0;
                            if ui
                                .add(egui::DragValue::new(&mut m).speed(0.5).range(0.0..=100.0))
                                .changed()
                            {
                                self.grid_config.margins.bottom = Millimeters(m);
                            }
                        });

                        ui.separator();
                        ui.label("Gutters (Spacing between photos):");
                        ui.horizontal(|ui| {
                            ui.label("Horizontal:");
                            let mut g = self.grid_config.gutters.horizontal.0;
                            if ui
                                .add(egui::DragValue::new(&mut g).speed(0.5).range(0.0..=50.0))
                                .changed()
                            {
                                self.grid_config.gutters.horizontal = Millimeters(g);
                            }
                            ui.label("Vertical:");
                            let mut g = self.grid_config.gutters.vertical.0;
                            if ui
                                .add(egui::DragValue::new(&mut g).speed(0.5).range(0.0..=50.0))
                                .changed()
                            {
                                self.grid_config.gutters.vertical = Millimeters(g);
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // --- Layout Verification ---
                    ui.group(|ui| {
                        ui.strong("🔍 Layout Verification");
                        ui.add_space(4.0);

                        match compute_layout(&self.grid_config) {
                            Ok(layout) => {
                                ui.colored_label(
                                    Color32::from_rgb(46, 160, 67),
                                    format!(
                                        "✓ Print Ready: {} photos fit on sheet",
                                        layout.count()
                                    ),
                                );
                                ui.label(format!(
                                    "Grid Bounds: {} × {}",
                                    layout.grid_width, layout.grid_height
                                ));
                                ui.label(format!(
                                    "Printable Area: {} × {}",
                                    layout.printable_width, layout.printable_height
                                ));
                            }
                            Err(err) => {
                                ui.colored_label(
                                    Color32::from_rgb(230, 80, 70),
                                    format!("⚠ Layout Error:\n{err}"),
                                );
                            }
                        }
                    });
                });
            });
    }

    /// Renders the bottom status bar.
    fn render_status_bar(&mut self, ui: &mut Ui) {
        Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let layout_res = compute_layout(&self.grid_config);
                    match layout_res {
                        Ok(geom) => {
                            ui.colored_label(
                                Color32::from_rgb(46, 160, 67),
                                format!("✓ {} Photos", geom.count()),
                            );
                        }
                        Err(_) => {
                            ui.colored_label(Color32::from_rgb(230, 80, 70), "⚠ Layout Overflow");
                        }
                    }

                    ui.separator();
                    ui.label(format!("Zoom: {:.0}%", self.canvas_zoom * 100.0));
                    ui.separator();
                    ui.label(format!("{}", self.dpi));
                    ui.separator();
                    ui.label(format!(
                        "Photo: {} × {}",
                        self.grid_config.photo.width, self.grid_config.photo.height
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Sheet: {} × {}",
                        self.grid_config.sheet.width, self.grid_config.sheet.height
                    ));
                });
            });
        });
    }

    /// Renders the central canvas: either interactive Crop Editor or Printable Sheet.
    fn render_central_canvas(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            if self.active_tool == ToolMode::Crop {
                self.render_crop_editor(ui);
            } else {
                self.render_sheet_canvas(ui);
            }
        });
    }

    /// Renders the interactive crop editor tool.
    fn render_crop_editor(&mut self, ui: &mut Ui) {
        let available_size = ui.available_size();
        let (rect, _response) =
            ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        // Dark workbench canvas background
        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 22, 28));

        let (src_w_u32, src_h_u32) = match &self.loaded_image {
            Some(img) => (img.width, img.height),
            None => {
                // Empty state when no photo loaded
                painter.text(
                    rect.center() - Vec2::new(0.0, 30.0),
                    egui::Align2::CENTER_CENTER,
                    "No Photograph Loaded",
                    egui::FontId::proportional(20.0),
                    Color32::from_rgb(180, 185, 195),
                );
                painter.text(
                    rect.center() + Vec2::new(0.0, 10.0),
                    egui::Align2::CENTER_CENTER,
                    "Import an image file or drag and drop a photo onto this window to crop.",
                    egui::FontId::proportional(13.0),
                    Color32::from_rgb(130, 135, 145),
                );
                return;
            }
        };

        let src_w = src_w_u32 as f32;
        let src_h = src_h_u32 as f32;

        // Top toolbar inside crop editor
        let bar_h = 42.0;
        let bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), bar_h));
        painter.rect_filled(bar_rect, 0.0, Color32::from_rgb(28, 30, 38));
        painter.line_segment(
            [bar_rect.left_bottom(), bar_rect.right_bottom()],
            Stroke::new(1.0, Color32::from_rgb(45, 48, 58)),
        );

        let target_aspect = self.grid_config.photo.aspect_ratio();

        // Toolbar UI elements
        let mut child_ui =
            ui.new_child(egui::UiBuilder::new().max_rect(bar_rect.shrink2(Vec2::new(8.0, 4.0))));
        child_ui.horizontal(|ui| {
            ui.strong("✂ Crop Photo");
            ui.separator();
            ui.label(format!(
                "Passport: {} × {} ({:.2}:1)",
                self.grid_config.photo.width, self.grid_config.photo.height, target_aspect
            ));

            ui.separator();
            if ui
                .button("⚡ Max Fit")
                .on_hover_text("Fit maximum centered crop box")
                .clicked()
            {
                self.crop_area = CropArea::default_for_aspect(src_w_u32, src_h_u32, target_aspect);
                self.texture_needs_update = true;
            }
            if ui
                .button("⊙ Center")
                .on_hover_text("Center crop box")
                .clicked()
            {
                let center_x = (1.0 - self.crop_area.width) / 2.0;
                let center_y = (1.0 - self.crop_area.height) / 2.0;
                self.crop_area.x = center_x.clamp(0.0, 1.0);
                self.crop_area.y = center_y.clamp(0.0, 1.0);
                self.crop_area.clamp_within_bounds();
                self.texture_needs_update = true;
            }
            if ui.button("🔍+").on_hover_text("Zoom In Crop").clicked() {
                self.zoom_crop(0.92);
            }
            if ui.button("🔍-").on_hover_text("Zoom Out Crop").clicked() {
                self.zoom_crop(1.08);
            }

            ui.separator();
            ui.checkbox(&mut self.show_biometric_guides, "Biometric Face Guides");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui::RichText::new("✓ Done (View Sheet)").strong())
                    .on_hover_text("Apply crop and return to sheet layout")
                    .clicked()
                {
                    self.active_tool = ToolMode::SelectPan;
                }
            });
        });

        // Viewable image workspace area
        let work_rect = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + bar_h), rect.max);

        let padding = 36.0;
        let fit_scale = ((work_rect.width() - padding * 2.0).max(10.0) / src_w)
            .min((work_rect.height() - padding * 2.0).max(10.0) / src_h);

        let disp_w = src_w * fit_scale;
        let disp_h = src_h * fit_scale;
        let disp_rect = Rect::from_center_size(work_rect.center(), Vec2::new(disp_w, disp_h));

        // Paint source image
        if let Some(tex) = &self.source_texture {
            painter.image(
                tex.id(),
                disp_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        // Compute crop frame screen coordinates
        let crop_screen_x = disp_rect.min.x + (self.crop_area.x * disp_w);
        let crop_screen_y = disp_rect.min.y + (self.crop_area.y * disp_h);
        let crop_screen_w = self.crop_area.width * disp_w;
        let crop_screen_h = self.crop_area.height * disp_h;

        let crop_rect = Rect::from_min_size(
            Pos2::new(crop_screen_x, crop_screen_y),
            Vec2::new(crop_screen_w, crop_screen_h),
        );

        // Dim area outside the crop rectangle
        let mask_color = Color32::from_rgba_unmultiplied(0, 0, 0, 165);
        // Top
        let top_mask =
            Rect::from_min_max(disp_rect.min, Pos2::new(disp_rect.max.x, crop_rect.min.y));
        if top_mask.is_positive() {
            painter.rect_filled(top_mask, 0.0, mask_color);
        }
        // Bottom
        let bottom_mask =
            Rect::from_min_max(Pos2::new(disp_rect.min.x, crop_rect.max.y), disp_rect.max);
        if bottom_mask.is_positive() {
            painter.rect_filled(bottom_mask, 0.0, mask_color);
        }
        // Left
        let left_mask = Rect::from_min_max(
            Pos2::new(disp_rect.min.x, crop_rect.min.y),
            Pos2::new(crop_rect.min.x, crop_rect.max.y),
        );
        if left_mask.is_positive() {
            painter.rect_filled(left_mask, 0.0, mask_color);
        }
        // Right
        let right_mask = Rect::from_min_max(
            Pos2::new(crop_rect.max.x, crop_rect.min.y),
            Pos2::new(disp_rect.max.x, crop_rect.max.y),
        );
        if right_mask.is_positive() {
            painter.rect_filled(right_mask, 0.0, mask_color);
        }

        // Draw crisp crop boundary
        painter.rect_stroke(
            crop_rect,
            0.0,
            Stroke::new(2.0, Color32::WHITE),
            egui::StrokeKind::Outside,
        );

        // Handles geometry
        let nw = crop_rect.left_top();
        let ne = crop_rect.right_top();
        let sw = crop_rect.left_bottom();
        let se = crop_rect.right_bottom();
        let handle_hit_radius = 24.0;

        let mouse_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_pressed = ui.input(|i| i.pointer.primary_pressed());

        if !primary_down {
            self.crop_drag_mode = CropDragMode::None;
        }

        let mut is_near_nw = false;
        let mut is_near_ne = false;
        let mut is_near_sw = false;
        let mut is_near_se = false;

        if let Some(pos) = mouse_pos {
            is_near_nw = (pos - nw).length() <= handle_hit_radius;
            is_near_ne = (pos - ne).length() <= handle_hit_radius;
            is_near_sw = (pos - sw).length() <= handle_hit_radius;
            is_near_se = (pos - se).length() <= handle_hit_radius;
            let is_inside = crop_rect.contains(pos);

            // Wheel zoom when hovering over work area
            if work_rect.contains(pos) {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.0 {
                    let zoom_factor = if scroll > 0.0 { 0.96 } else { 1.04 };
                    self.zoom_crop(zoom_factor);
                    ui.ctx().request_repaint();
                }
            }

            // Initiate drag mode when mouse is clicked
            if primary_pressed && work_rect.contains(pos) {
                if is_near_nw {
                    self.crop_drag_mode = CropDragMode::Corner(CropCorner::TopLeft);
                } else if is_near_ne {
                    self.crop_drag_mode = CropDragMode::Corner(CropCorner::TopRight);
                } else if is_near_sw {
                    self.crop_drag_mode = CropDragMode::Corner(CropCorner::BottomLeft);
                } else if is_near_se {
                    self.crop_drag_mode = CropDragMode::Corner(CropCorner::BottomRight);
                } else if is_inside {
                    self.crop_drag_mode = CropDragMode::Move;
                }
            }

            // Set responsive cursor icon
            match self.crop_drag_mode {
                CropDragMode::Corner(CropCorner::TopLeft | CropCorner::BottomRight) => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }
                CropDragMode::Corner(CropCorner::TopRight | CropCorner::BottomLeft) => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNeSw);
                }
                CropDragMode::Move => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                CropDragMode::None => {
                    if is_near_nw || is_near_se {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                    } else if is_near_ne || is_near_sw {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNeSw);
                    } else if is_inside {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                }
            }
        }

        // Draw tactile corner bracket handles with visual knob indicators
        draw_crop_corner(
            &painter,
            nw,
            Vec2::new(1.0, 1.0),
            is_near_nw
                || matches!(
                    self.crop_drag_mode,
                    CropDragMode::Corner(CropCorner::TopLeft)
                ),
        );
        draw_crop_corner(
            &painter,
            ne,
            Vec2::new(-1.0, 1.0),
            is_near_ne
                || matches!(
                    self.crop_drag_mode,
                    CropDragMode::Corner(CropCorner::TopRight)
                ),
        );
        draw_crop_corner(
            &painter,
            sw,
            Vec2::new(1.0, -1.0),
            is_near_sw
                || matches!(
                    self.crop_drag_mode,
                    CropDragMode::Corner(CropCorner::BottomLeft)
                ),
        );
        draw_crop_corner(
            &painter,
            se,
            Vec2::new(-1.0, -1.0),
            is_near_se
                || matches!(
                    self.crop_drag_mode,
                    CropDragMode::Corner(CropCorner::BottomRight)
                ),
        );

        // Center crosshair for precise face alignment
        let center = crop_rect.center();
        let ch_len = 10.0;
        let ch_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 90));
        painter.line_segment(
            [
                center - Vec2::new(ch_len, 0.0),
                center + Vec2::new(ch_len, 0.0),
            ],
            ch_stroke,
        );
        painter.line_segment(
            [
                center - Vec2::new(0.0, ch_len),
                center + Vec2::new(0.0, ch_len),
            ],
            ch_stroke,
        );

        // Biometric passport alignment guides
        if self.show_biometric_guides {
            let guide_stroke =
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 200, 255, 140));
            let eye_stroke = Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 215, 0, 180));

            // Vertical center line (face symmetry)
            let cx = crop_rect.center().x;
            painter.line_segment(
                [
                    Pos2::new(cx, crop_rect.min.y),
                    Pos2::new(cx, crop_rect.max.y),
                ],
                guide_stroke,
            );

            // Crown guide (top of head: ~12% from top)
            let y_crown = crop_rect.min.y + (crop_screen_h * 0.12);
            painter.line_segment(
                [
                    Pos2::new(crop_rect.min.x, y_crown),
                    Pos2::new(crop_rect.max.x, y_crown),
                ],
                guide_stroke,
            );
            painter.text(
                Pos2::new(crop_rect.min.x + 4.0, y_crown - 2.0),
                egui::Align2::LEFT_BOTTOM,
                "Crown (Top of Head)",
                egui::FontId::proportional(10.0),
                Color32::from_rgba_unmultiplied(100, 200, 255, 200),
            );

            // Eye line guide (typically 56-69% from bottom / ~40% from top)
            let y_eyes = crop_rect.max.y - (crop_screen_h * 0.60);
            painter.line_segment(
                [
                    Pos2::new(crop_rect.min.x, y_eyes),
                    Pos2::new(crop_rect.max.x, y_eyes),
                ],
                eye_stroke,
            );
            painter.text(
                Pos2::new(crop_rect.min.x + 4.0, y_eyes - 2.0),
                egui::Align2::LEFT_BOTTOM,
                "Eye Level (56-69%)",
                egui::FontId::proportional(10.0),
                Color32::from_rgba_unmultiplied(255, 215, 0, 220),
            );

            // Chin guide (~25% from bottom)
            let y_chin = crop_rect.max.y - (crop_screen_h * 0.25);
            painter.line_segment(
                [
                    Pos2::new(crop_rect.min.x, y_chin),
                    Pos2::new(crop_rect.max.x, y_chin),
                ],
                guide_stroke,
            );
            painter.text(
                Pos2::new(crop_rect.min.x + 4.0, y_chin - 2.0),
                egui::Align2::LEFT_BOTTOM,
                "Chin Line",
                egui::FontId::proportional(10.0),
                Color32::from_rgba_unmultiplied(100, 200, 255, 200),
            );
        }

        // Real-time dimension badge above crop frame
        let badge_text = format!(
            "{} × {} mm",
            self.grid_config.photo.width.0, self.grid_config.photo.height.0
        );
        let badge_pos = Pos2::new(crop_rect.center().x, crop_rect.min.y - 12.0);
        if badge_pos.y >= work_rect.min.y + 12.0 {
            painter.text(
                badge_pos,
                egui::Align2::CENTER_CENTER,
                badge_text,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(220, 225, 235),
            );
        }

        // Smooth drag execution
        if primary_down && self.crop_drag_mode != CropDragMode::None && disp_w > 0.0 && disp_h > 0.0
        {
            let delta = ui.input(|i| i.pointer.delta());
            if delta.length_sq() > 0.0 {
                let delta_norm_x = delta.x / disp_w;
                let delta_norm_y = delta.y / disp_h;
                let desired_norm_aspect = target_aspect / (src_w / src_h);

                match self.crop_drag_mode {
                    CropDragMode::Move => {
                        self.crop_area.x += delta_norm_x;
                        self.crop_area.y += delta_norm_y;
                        self.crop_area.clamp_within_bounds();
                    }
                    CropDragMode::Corner(corner) => {
                        self.crop_area.resize_from_corner(
                            corner,
                            delta_norm_x,
                            delta_norm_y,
                            desired_norm_aspect,
                        );
                    }
                    CropDragMode::None => {}
                }
                self.texture_needs_update = true;
            }
            ui.ctx().request_repaint();
        }
    }

    /// Renders the central printable sheet canvas with photos and cut marks.
    fn render_sheet_canvas(&mut self, ui: &mut Ui) {
        let available_size = ui.available_size();
        let (rect, response) =
            ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        // Pan handling
        if response.dragged() {
            self.canvas_pan += response.drag_delta();
        }

        // Zoom handling via mouse wheel scroll
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.0 {
            let factor = (1.0 + scroll * 0.002).clamp(0.8, 1.25);
            self.canvas_zoom = (self.canvas_zoom * factor).clamp(0.2, 5.0);
        }

        let painter = ui.painter_at(rect);

        // Draw dark workbench canvas background
        painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 32, 38));

        // Determine scale: mm to screen pixels
        let sheet_w_mm = self.grid_config.sheet.width.0.max(1.0);
        let sheet_h_mm = self.grid_config.sheet.height.0.max(1.0);

        let padding = 40.0;
        let fit_scale_x = (rect.width() - padding * 2.0).max(10.0) / sheet_w_mm;
        let fit_scale_y = (rect.height() - padding * 2.0).max(10.0) / sheet_h_mm;
        let base_scale = fit_scale_x.min(fit_scale_y);
        let px_per_mm = base_scale * self.canvas_zoom;

        let sheet_pixel_w = sheet_w_mm * px_per_mm;
        let sheet_pixel_h = sheet_h_mm * px_per_mm;

        // Center of the canvas plus user pan offset
        let sheet_origin_x = rect.center().x - (sheet_pixel_w / 2.0) + self.canvas_pan.x;
        let sheet_origin_y = rect.center().y - (sheet_pixel_h / 2.0) + self.canvas_pan.y;

        let sheet_rect = Rect::from_min_size(
            Pos2::new(sheet_origin_x, sheet_origin_y),
            Vec2::new(sheet_pixel_w, sheet_pixel_h),
        );

        // Sheet drop shadow
        let shadow_offset = 6.0;
        painter.rect_filled(
            sheet_rect.translate(Vec2::new(shadow_offset, shadow_offset)),
            4.0,
            Color32::from_rgba_premultiplied(0, 0, 0, 60),
        );

        // Sheet paper background (pure white)
        painter.rect_filled(sheet_rect, 2.0, Color32::WHITE);
        painter.rect_stroke(
            sheet_rect,
            2.0,
            Stroke::new(1.0, Color32::from_rgb(180, 185, 195)),
            egui::StrokeKind::Outside,
        );

        // Printable area & margins
        if self.show_margins {
            let m_left_px = self.grid_config.margins.left.0 * px_per_mm;
            let m_top_px = self.grid_config.margins.top.0 * px_per_mm;
            let m_right_px = self.grid_config.margins.right.0 * px_per_mm;
            let m_bottom_px = self.grid_config.margins.bottom.0 * px_per_mm;

            let printable_rect = Rect::from_min_max(
                Pos2::new(sheet_rect.min.x + m_left_px, sheet_rect.min.y + m_top_px),
                Pos2::new(
                    sheet_rect.max.x - m_right_px,
                    sheet_rect.max.y - m_bottom_px,
                ),
            );

            if printable_rect.is_positive() {
                painter.rect_stroke(
                    printable_rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(210, 215, 225)),
                    egui::StrokeKind::Inside,
                );
            }
        }

        // Render photo slots from computed layout
        match compute_layout(&self.grid_config) {
            Ok(layout) => {
                self.paint_slots(&painter, &sheet_rect, px_per_mm, &layout);

                // Slot click handling: clicking a slot either opens crop tool or imports photo
                if let Some(pos) = response
                    .interact_pointer_pos()
                    .filter(|_| response.clicked())
                {
                    for (idx, slot) in layout.slots.iter().enumerate() {
                        let slot_x = sheet_rect.min.x + (slot.x.0 * px_per_mm);
                        let slot_y = sheet_rect.min.y + (slot.y.0 * px_per_mm);
                        let slot_w = slot.width.0 * px_per_mm;
                        let slot_h = slot.height.0 * px_per_mm;
                        let slot_rect = Rect::from_min_size(
                            Pos2::new(slot_x, slot_y),
                            Vec2::new(slot_w, slot_h),
                        );

                        if slot_rect.contains(pos) {
                            self.selected_slot = Some(idx);
                            if self.loaded_image.is_some() {
                                self.active_tool = ToolMode::Crop;
                            } else {
                                self.import_image_dialog();
                            }
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                // Paint overflow warning overlay on the sheet
                painter.rect_filled(
                    sheet_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(220, 50, 50, 40),
                );
                painter.text(
                    sheet_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("⚠ Layout Overflow\n{err}"),
                    egui::FontId::proportional(15.0),
                    Color32::from_rgb(180, 20, 20),
                );
            }
        }

        // Draw sheet dimension annotations
        if self.show_dimensions {
            let dim_text = format!(
                "{} × {} ({:.1} × {:.1} in)",
                self.grid_config.sheet.width,
                self.grid_config.sheet.height,
                self.grid_config.sheet.width.to_inches().0,
                self.grid_config.sheet.height.to_inches().0
            );
            painter.text(
                Pos2::new(sheet_rect.center().x, sheet_rect.max.y + 16.0),
                egui::Align2::CENTER_TOP,
                dim_text,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(160, 165, 175),
            );
        }
    }

    /// Paints photo slots and cropped images onto the sheet canvas.
    fn paint_slots(
        &self,
        painter: &egui::Painter,
        sheet_rect: &Rect,
        px_per_mm: f32,
        layout: &LayoutGeometry,
    ) {
        for (idx, slot) in layout.slots.iter().enumerate() {
            let slot_x = sheet_rect.min.x + (slot.x.0 * px_per_mm);
            let slot_y = sheet_rect.min.y + (slot.y.0 * px_per_mm);
            let slot_w = slot.width.0 * px_per_mm;
            let slot_h = slot.height.0 * px_per_mm;

            let photo_rect =
                Rect::from_min_size(Pos2::new(slot_x, slot_y), Vec2::new(slot_w, slot_h));

            if let Some(tex) = &self.cropped_texture {
                // Render cropped image
                painter.image(
                    tex.id(),
                    photo_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                // Slot border outline
                painter.rect_stroke(
                    photo_rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(180, 190, 205)),
                    egui::StrokeKind::Inside,
                );
            } else {
                // Photo card placeholder background
                painter.rect_filled(photo_rect, 0.0, Color32::from_rgb(245, 247, 250));
                painter.rect_stroke(
                    photo_rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(100, 130, 180)),
                    egui::StrokeKind::Inside,
                );

                let label = format!(
                    "#{}\n{} × {}\n+ Click to Load",
                    idx + 1,
                    slot.width,
                    slot.height
                );
                painter.text(
                    photo_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(120, 140, 170),
                );
            }

            // Cut marks at corners
            if self.show_cut_marks {
                let mark_len = 6.0;
                let stroke = Stroke::new(1.0, Color32::from_rgb(70, 75, 85));

                // Top-left
                painter.line_segment(
                    [photo_rect.min, photo_rect.min + Vec2::new(mark_len, 0.0)],
                    stroke,
                );
                painter.line_segment(
                    [photo_rect.min, photo_rect.min + Vec2::new(0.0, mark_len)],
                    stroke,
                );

                // Top-right
                let tr = Pos2::new(photo_rect.max.x, photo_rect.min.y);
                painter.line_segment([tr, tr - Vec2::new(mark_len, 0.0)], stroke);
                painter.line_segment([tr, tr + Vec2::new(0.0, mark_len)], stroke);

                // Bottom-left
                let bl = Pos2::new(photo_rect.min.x, photo_rect.max.y);
                painter.line_segment([bl, bl + Vec2::new(mark_len, 0.0)], stroke);
                painter.line_segment([bl, bl - Vec2::new(0.0, mark_len)], stroke);

                // Bottom-right
                painter.line_segment(
                    [photo_rect.max, photo_rect.max - Vec2::new(mark_len, 0.0)],
                    stroke,
                );
                painter.line_segment(
                    [photo_rect.max, photo_rect.max - Vec2::new(0.0, mark_len)],
                    stroke,
                );
            }
        }
    }

    /// Renders modal dialogs such as About.
    fn render_dialogs(&mut self, ui: &mut Ui) {
        if self.show_about_dialog {
            egui::Window::new("About Imprint")
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(320.0, 200.0))
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Imprint");
                        ui.label("Passport & ID Photo Print Studio");
                        ui.label("Version 0.1.0");
                        ui.add_space(8.0);
                        ui.label(
                            "High-precision photo preparation, grid layout,\nand print-ready export system.",
                        );
                        ui.add_space(12.0);
                        if ui.button("Close").clicked() {
                            self.show_about_dialog = false;
                        }
                    });
                });
        }
    }
}

/// Draws an L-shaped corner bracket handle with high contrast shadow and highlight dot.
fn draw_crop_corner(painter: &egui::Painter, pos: Pos2, dir: Vec2, is_hovered: bool) {
    let len = 18.0;
    let thickness = 4.0;
    let color = if is_hovered {
        Color32::from_rgb(255, 215, 0)
    } else {
        Color32::WHITE
    };
    let shadow = Stroke::new(thickness + 2.0, Color32::from_black_alpha(140));
    painter.line_segment([pos, pos + Vec2::new(len * dir.x, 0.0)], shadow);
    painter.line_segment([pos, pos + Vec2::new(0.0, len * dir.y)], shadow);
    let stroke = Stroke::new(thickness, color);
    painter.line_segment([pos, pos + Vec2::new(len * dir.x, 0.0)], stroke);
    painter.line_segment([pos, pos + Vec2::new(0.0, len * dir.y)], stroke);
    painter.circle_filled(pos, 3.5, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::DynamicImage;

    #[test]
    fn test_default_app_state() {
        let app = ImprintApp::default();
        assert_eq!(app.photo_preset, PhotoPreset::UsPassport);
        assert_eq!(app.sheet_preset, SheetPreset::Photo4x6);
        assert_eq!(app.sheet_orientation, SheetOrientation::Portrait);
        assert_eq!(app.dpi, Dpi::STANDARD);
        assert_eq!(app.active_tool, ToolMode::SelectPan);
        assert!(app.loaded_image.is_none());
        assert_eq!(app.crop_area, CropArea::default());
        assert_eq!(app.export_target, ExportTarget::FullSheet);
        assert_eq!(app.export_format, ExportFormat::Pdf);
    }

    #[test]
    fn test_apply_sheet_and_photo_presets() {
        let mut app = ImprintApp {
            sheet_preset: SheetPreset::A4,
            sheet_orientation: SheetOrientation::Landscape,
            ..Default::default()
        };
        app.apply_sheet_preset();

        let a4_landscape = SheetPreset::A4
            .oriented_dimensions(SheetOrientation::Landscape)
            .unwrap();
        assert_eq!(app.grid_config.sheet, a4_landscape);

        app.photo_preset = PhotoPreset::EuSchengen;
        app.apply_photo_preset();
        let eu_dims = PhotoPreset::EuSchengen.dimensions().unwrap();
        assert_eq!(app.grid_config.photo, eu_dims);
    }

    #[test]
    fn test_zoom_crop_clamping() {
        let mut app = ImprintApp::default();
        // Zoom way in
        for _ in 0..20 {
            app.zoom_crop(0.5);
        }
        assert!(app.crop_area.width >= 0.05);
        assert!(app.crop_area.height >= 0.05);
        assert!(app.crop_area.x >= 0.0);
        assert!(app.crop_area.y >= 0.0);
        assert!(app.crop_area.x + app.crop_area.width <= 1.0001);

        // Zoom way out
        for _ in 0..20 {
            app.zoom_crop(2.0);
        }
        assert!(app.crop_area.width <= 1.0);
        assert!(app.crop_area.height <= 1.0);
        assert!(app.crop_area.x >= 0.0);
        assert!(app.crop_area.y >= 0.0);
    }

    #[test]
    fn test_photo_preset_adjusts_crop_aspect_ratio_when_image_loaded() {
        let mut app = ImprintApp::default();
        let dummy_img = DynamicImage::new_rgb8(1000, 1000);
        let loaded = LoadedImage::from_dynamic_image(dummy_img, "test.jpg");
        app.loaded_image = Some(loaded);
        app.crop_area = CropArea::default_for_aspect(1000, 1000, 1.0); // 1:1 aspect

        // Switch to Schengen (35x45 mm -> aspect 35/45 = ~0.777)
        app.photo_preset = PhotoPreset::EuSchengen;
        app.apply_photo_preset();

        let crop_aspect = (app.crop_area.width * 1000.0) / (app.crop_area.height * 1000.0);
        let target_aspect = 35.0 / 45.0;
        assert!((crop_aspect - target_aspect).abs() < 1e-2);
        assert!(app.texture_needs_update);
    }

    #[test]
    fn test_auto_fit_grid() {
        let mut app = ImprintApp::default();
        app.auto_fit_grid();
        // On 4x6 inch (101.6 x 152.4 mm) with 2mm margins and 2mm gutters,
        // fitting 50.8x50.8 mm photos
        // printable_w = 101.6 - 4 = 97.6 mm -> fits 1 col of 50.8 mm + 2mm gutter? (50.8 * 2 + 2 = 103.6 > 97.6) -> 1 col
        // printable_h = 152.4 - 4 = 148.4 mm -> fits 2 rows (50.8 * 2 + 2 = 103.6 < 148.4) -> 2 rows
        assert!(app.grid_config.rows >= 1);
        assert!(app.grid_config.cols >= 1);
    }
}
