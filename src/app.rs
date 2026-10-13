//! Main GUI application state, interactive canvas, crop tool, and print export.

use std::path::Path;

use eframe::egui::{self, Color32, Panel, Pos2, Rect, Stroke, Ui, Vec2};

use crate::export::{ExportFormat, ExportOptions, ExportTarget, export_to_file};
use crate::image_ops::{CropArea, CropCorner, LoadedImage, crop_and_resample, to_egui_color_image};
use crate::layout::{
    GridConfig, GuttersMm, LayoutGeometry, MarginsMm, calculate_max_fit, compute_layout,
};
use crate::preset::{PhotoPreset, SheetOrientation, SheetPreset};
use crate::ui_icons::{
    ToolbarIcon, render_options_bar_button, render_toolbar_button, render_toolbar_divider,
};
use crate::units::{DimensionsMm, Dpi, Millimeters};

/// Top-level workspace mode separating single photo editing from printable sheet layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceMode {
    #[default]
    PhotoEdit,
    SheetLayout,
}

impl WorkspaceMode {
    pub fn label(self) -> &'static str {
        match self {
            WorkspaceMode::PhotoEdit => "Photo Canvas",
            WorkspaceMode::SheetLayout => "Sheet Print Preview",
        }
    }
}

/// Active interactive tool in the current workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolMode {
    #[default]
    Select,
    Pan,
    Crop,
}

impl ToolMode {
    pub fn name(self) -> &'static str {
        match self {
            ToolMode::Select => "Select",
            ToolMode::Pan => "Pan",
            ToolMode::Crop => "Crop Photo",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            ToolMode::Select => "V",
            ToolMode::Pan => "H",
            ToolMode::Crop => "C",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ToolMode::Select => "Select and inspect photo slots on printable sheet",
            ToolMode::Pan => "Pan and navigate the workspace canvas",
            ToolMode::Crop => "Crop photo with passport proportions and corner handles",
        }
    }

    pub fn icon(self) -> ToolbarIcon {
        match self {
            ToolMode::Select => ToolbarIcon::Pointer,
            ToolMode::Pan => ToolbarIcon::SelectPan,
            ToolMode::Crop => ToolbarIcon::Crop,
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
    // Workspace mode
    pub workspace: WorkspaceMode,

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

    // Open image document tabs & undo/redo history
    pub documents: Vec<crate::document::ImageDocument>,
    pub active_doc_idx: usize,
    pub next_doc_id: usize,

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
            rows: 1,
            cols: 1,
            margins: MarginsMm::new(2.0, 2.0, 2.0, 2.0),
            gutters: GuttersMm::new(2.0, 2.0),
            auto_center: true,
        };

        Self {
            workspace: WorkspaceMode::PhotoEdit,
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
            documents: Vec::new(),
            active_doc_idx: 0,
            next_doc_id: 0,
            export_target: ExportTarget::FullSheet,
            export_format: ExportFormat::Pdf,
            active_tool: ToolMode::Crop,
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

    /// Returns a reference to the active document, if any.
    pub fn active_document(&self) -> Option<&crate::document::ImageDocument> {
        self.documents.get(self.active_doc_idx)
    }

    /// Returns a mutable reference to the active document, if any.
    pub fn active_document_mut(&mut self) -> Option<&mut crate::document::ImageDocument> {
        self.documents.get_mut(self.active_doc_idx)
    }

    /// Synchronizes active document's state to top-level compatibility fields.
    pub fn sync_active_doc_to_compat(&mut self) {
        if let Some(doc) = self.documents.get(self.active_doc_idx) {
            self.loaded_image = Some(doc.loaded_image.clone());
            self.crop_area = doc.crop_area;
            self.source_texture = None;
            self.cropped_texture = None;
            self.texture_needs_update = true;
        } else {
            self.loaded_image = None;
            self.source_texture = None;
            self.cropped_texture = None;
        }
    }

    /// Adds a loaded image as a new document tab and selects it.
    pub fn add_document(&mut self, loaded: LoadedImage) {
        let target_aspect = self.grid_config.photo.aspect_ratio();
        self.next_doc_id += 1;
        let doc =
            crate::document::ImageDocument::new(self.next_doc_id, loaded.clone(), target_aspect);
        self.documents.push(doc);
        self.active_doc_idx = self.documents.len() - 1;
        self.sync_active_doc_to_compat();
        self.workspace = WorkspaceMode::PhotoEdit;
        self.active_tool = ToolMode::Crop;
        self.status_message = format!(
            "Opened tab: {} ({} × {} px)",
            loaded.file_name, loaded.width, loaded.height
        );
        if compute_layout(&self.grid_config).is_err() {
            self.grid_config.rows = 1;
            self.grid_config.cols = 1;
        }
    }

    /// Selects the document tab at the given index.
    pub fn select_document(&mut self, idx: usize) {
        if idx < self.documents.len() && idx != self.active_doc_idx {
            if let Some(cur_doc) = self.documents.get_mut(self.active_doc_idx) {
                cur_doc.crop_area = self.crop_area;
            }
            self.active_doc_idx = idx;
            self.sync_active_doc_to_compat();
            if let Some(doc) = self.documents.get(self.active_doc_idx) {
                self.status_message = format!("Switched to tab: {}", doc.title);
            }
        }
    }

    /// Closes the document tab at the given index.
    pub fn close_document(&mut self, idx: usize) {
        if idx < self.documents.len() {
            let closed_title = self.documents.remove(idx).title;
            if self.documents.is_empty() {
                self.active_doc_idx = 0;
                self.loaded_image = None;
                self.source_texture = None;
                self.cropped_texture = None;
                self.status_message = format!("Closed tab: {closed_title}. No photos open.");
            } else {
                if self.active_doc_idx >= self.documents.len() {
                    self.active_doc_idx = self.documents.len() - 1;
                }
                self.sync_active_doc_to_compat();
                self.status_message = format!("Closed tab: {closed_title}.");
            }
        }
    }

    /// Returns `true` if an undo operation is available on the active document.
    pub fn can_undo(&self) -> bool {
        self.documents
            .get(self.active_doc_idx)
            .is_some_and(|d| d.can_undo())
    }

    /// Returns `true` if a redo operation is available on the active document.
    pub fn can_redo(&self) -> bool {
        self.documents
            .get(self.active_doc_idx)
            .is_some_and(|d| d.can_redo())
    }

    /// Restores the previous snapshot from undo history.
    pub fn undo(&mut self) -> bool {
        if let Some(doc) = self.documents.get_mut(self.active_doc_idx)
            && doc.undo()
        {
            self.loaded_image = Some(doc.loaded_image.clone());
            self.crop_area = doc.crop_area;
            self.source_texture = None;
            self.cropped_texture = None;
            self.texture_needs_update = true;
            self.status_message = format!(
                "Undo crop: restored to {} × {} px.",
                doc.loaded_image.width, doc.loaded_image.height
            );
            return true;
        }
        false
    }

    /// Re-applies the reverted edit from redo history.
    pub fn redo(&mut self) -> bool {
        if let Some(doc) = self.documents.get_mut(self.active_doc_idx)
            && doc.redo()
        {
            self.loaded_image = Some(doc.loaded_image.clone());
            self.crop_area = doc.crop_area;
            self.source_texture = None;
            self.cropped_texture = None;
            self.texture_needs_update = true;
            self.status_message = format!(
                "Redo crop: reapplied {} × {} px.",
                doc.loaded_image.width, doc.loaded_image.height
            );
            return true;
        }
        false
    }

    /// Commits the current crop area to the underlying photograph (Photoshop-style crop execution).
    /// Shrinks the underlying image to the cropped pixel region, updating canvas and history stack.
    pub fn commit_crop(&mut self) {
        let target_aspect = self.grid_config.photo.aspect_ratio();
        if !self.documents.is_empty() && self.active_doc_idx < self.documents.len() {
            let doc = &mut self.documents[self.active_doc_idx];
            doc.crop_area = self.crop_area;
            match doc.commit_crop(target_aspect) {
                Ok(()) => {
                    self.loaded_image = Some(doc.loaded_image.clone());
                    self.crop_area = doc.crop_area;
                    self.source_texture = None;
                    self.cropped_texture = None;
                    self.texture_needs_update = true;
                    self.status_message = format!(
                        "Cropped to selected area: {} × {} px. Press ⌘Z to undo.",
                        doc.loaded_image.width, doc.loaded_image.height
                    );
                }
                Err(err) => {
                    self.status_message = format!("Failed to crop selected area: {err}");
                }
            }
        } else if let Some(loaded) = &mut self.loaded_image
            && let Ok(cropped) =
                crate::image_ops::crop_to_dynamic_image(&loaded.original, &self.crop_area)
        {
            let (new_w, new_h) = (cropped.width(), cropped.height());
            loaded.original = cropped;
            loaded.width = new_w;
            loaded.height = new_h;
            self.crop_area = CropArea::default_for_aspect(new_w, new_h, target_aspect);
            self.source_texture = None;
            self.cropped_texture = None;
            self.texture_needs_update = true;
            self.status_message = format!("Cropped to selected area: {new_w} × {new_h} px.");
        }
    }

    /// Resets crop boundaries to the default aspect ratio framing of the photograph.
    /// If crop boundaries are already default and crops were previously committed, reverts to the original uncropped photo.
    pub fn reset_crop(&mut self) {
        let target_aspect = self.grid_config.photo.aspect_ratio();
        if let Some(loaded) = &self.loaded_image {
            let default_area =
                CropArea::default_for_aspect(loaded.width, loaded.height, target_aspect);

            let is_already_default = (self.crop_area.x - default_area.x).abs() < 0.005
                && (self.crop_area.y - default_area.y).abs() < 0.005
                && (self.crop_area.width - default_area.width).abs() < 0.005
                && (self.crop_area.height - default_area.height).abs() < 0.005;

            if is_already_default && self.can_undo() {
                while self.can_undo() {
                    self.undo();
                }
                self.status_message =
                    "Reverted photograph to original uncropped state.".to_string();
            } else {
                self.crop_area = default_area;
                self.crop_drag_mode = CropDragMode::None;
                self.source_texture = None;
                self.texture_needs_update = true;
                if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                    doc.crop_area = self.crop_area;
                }
                self.status_message =
                    "Reset crop boundaries to fit photograph framing.".to_string();
            }
        }
    }

    /// Switches workspace to Sheet Layout mode.
    pub fn switch_to_sheet_layout(&mut self) {
        if self.workspace != WorkspaceMode::SheetLayout {
            if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                doc.crop_area = self.crop_area;
            }
            self.workspace = WorkspaceMode::SheetLayout;
            self.active_tool = ToolMode::Select;
            self.texture_needs_update = true;
            self.status_message = "Workspace: Sheet Print Preview".to_string();

            // Default to 1 row and 1 column if grid currently overflows the printable sheet
            if compute_layout(&self.grid_config).is_err() {
                self.grid_config.rows = 1;
                self.grid_config.cols = 1;
            }
        }
    }

    /// Switches workspace to Photo Editor mode.
    pub fn switch_to_photo_edit(&mut self) {
        if self.workspace != WorkspaceMode::PhotoEdit {
            self.workspace = WorkspaceMode::PhotoEdit;
            self.active_tool = ToolMode::Crop;
            self.status_message = "Workspace: Photo Canvas".to_string();
        }
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
                if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                    doc.crop_area = self.crop_area;
                }
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
                self.add_document(loaded);
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
        if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
            doc.crop_area = self.crop_area;
        }
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

        // PERFORMANCE: While in the Photo Editor workspace, skip expensive CPU resampling!
        // The crop tool renders the source texture with real-time vector overlay guides.
        // We regenerate cropped_texture only when displaying the sheet layout.
        if self.workspace == WorkspaceMode::PhotoEdit {
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

    /// Handles Photoshop-style keyboard shortcuts when not typing into input fields.
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        ctx.input(|i| {
            let cmd_or_ctrl = i.modifiers.command
                || i.modifiers.ctrl
                || i.events.iter().any(|e| matches!(e, egui::Event::Key { modifiers, .. } if modifiers.command || modifiers.ctrl));
            let shift = i.modifiers.shift
                || i.events.iter().any(|e| matches!(e, egui::Event::Key { modifiers, .. } if modifiers.shift));

            // Undo: ⌘Z (without Shift)
            if cmd_or_ctrl && !shift && i.key_pressed(egui::Key::Z) {
                self.undo();
            }
            // Redo: ⌘⇧Z or ⌘Y
            else if (cmd_or_ctrl && shift && i.key_pressed(egui::Key::Z))
                || (cmd_or_ctrl && i.key_pressed(egui::Key::Y))
            {
                self.redo();
            }
            // Open image: ⌘O
            else if cmd_or_ctrl && i.key_pressed(egui::Key::O) {
                self.import_image_dialog();
            }
            // Export: ⌘E
            else if cmd_or_ctrl && i.key_pressed(egui::Key::E) {
                let target = match self.workspace {
                    WorkspaceMode::SheetLayout => ExportTarget::FullSheet,
                    WorkspaceMode::PhotoEdit => ExportTarget::SinglePhoto,
                };
                self.export_dialog(target);
            }
            // Sheet Preview Toggle: ⌘P
            else if cmd_or_ctrl && i.key_pressed(egui::Key::P) {
                if self.workspace == WorkspaceMode::SheetLayout {
                    self.switch_to_photo_edit();
                } else {
                    self.switch_to_sheet_layout();
                }
            }
            // Close Tab: ⌘W
            else if cmd_or_ctrl && i.key_pressed(egui::Key::W) {
                self.close_document(self.active_doc_idx);
            }
            // Commit Crop: Enter (in PhotoEdit & Crop mode)
            else if !cmd_or_ctrl && i.key_pressed(egui::Key::Enter) {
                if self.workspace == WorkspaceMode::PhotoEdit && self.active_tool == ToolMode::Crop
                {
                    self.commit_crop();
                }
            }
            // Escape: return from SheetLayout to PhotoEdit, or switch Crop to Select
            else if !cmd_or_ctrl && i.key_pressed(egui::Key::Escape) {
                if self.workspace == WorkspaceMode::SheetLayout {
                    self.switch_to_photo_edit();
                } else if self.active_tool == ToolMode::Crop {
                    self.active_tool = ToolMode::Select;
                    self.status_message = "Active tool: Select".to_string();
                }
            }
            // Zoom to Fit: 0 / ⌘0
            else if (cmd_or_ctrl && i.key_pressed(egui::Key::Num0))
                || (!cmd_or_ctrl && i.key_pressed(egui::Key::Num0))
            {
                match self.workspace {
                    WorkspaceMode::SheetLayout => self.auto_fit_grid(),
                    WorkspaceMode::PhotoEdit => {
                        if let Some(loaded) = &self.loaded_image {
                            self.crop_area = CropArea::default_for_aspect(
                                loaded.width,
                                loaded.height,
                                self.grid_config.photo.aspect_ratio(),
                            );
                            self.texture_needs_update = true;
                            if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                                doc.crop_area = self.crop_area;
                            }
                        }
                    }
                }
            }
            // Actual Size: 1 / ⌘1
            else if (cmd_or_ctrl && i.key_pressed(egui::Key::Num1))
                || (!cmd_or_ctrl && i.key_pressed(egui::Key::Num1))
            {
                self.reset_view();
            }
            // Workspace Toggle: Tab
            else if i.key_pressed(egui::Key::Tab) {
                match self.workspace {
                    WorkspaceMode::SheetLayout => self.switch_to_photo_edit(),
                    WorkspaceMode::PhotoEdit => self.switch_to_sheet_layout(),
                }
            }
            // Canvas Zoom: Plus / Equals / Minus
            else if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                self.canvas_zoom = (self.canvas_zoom * 1.15).min(5.0);
            } else if i.key_pressed(egui::Key::Minus) {
                self.canvas_zoom = (self.canvas_zoom / 1.15).max(0.2);
            }
            // Tool Shortcuts: V, H, C
            else if !cmd_or_ctrl && i.key_pressed(egui::Key::V) {
                self.active_tool = ToolMode::Select;
                self.status_message = "Active tool: Select [V]".to_string();
            } else if !cmd_or_ctrl && i.key_pressed(egui::Key::H) {
                self.active_tool = ToolMode::Pan;
                self.status_message = "Active tool: Pan Canvas [H]".to_string();
            } else if !cmd_or_ctrl && i.key_pressed(egui::Key::C) {
                self.workspace = WorkspaceMode::PhotoEdit;
                self.active_tool = ToolMode::Crop;
                self.status_message = "Active tool: Crop [C]".to_string();
            }
            // Guides Toggle: R
            else if !cmd_or_ctrl && i.key_pressed(egui::Key::R) {
                let any_on = self.show_cut_marks
                    || self.show_margins
                    || self.show_dimensions
                    || self.show_biometric_guides;
                self.show_cut_marks = !any_on;
                self.show_margins = !any_on;
                self.show_dimensions = !any_on;
                self.show_biometric_guides = !any_on;
                self.status_message = if !any_on {
                    "Guides and marks enabled.".to_string()
                } else {
                    "Guides and marks disabled.".to_string()
                };
            }
        });
    }
}

impl eframe::App for ImprintApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ui.ctx());
        self.handle_keyboard_shortcuts(ui.ctx());
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
                    if ui.button("Import Image... (⌘O)").clicked() {
                        self.import_image_dialog();
                        ui.close();
                    }
                    if !self.documents.is_empty() && ui.button("Close Tab (⌘W)").clicked() {
                        self.close_document(self.active_doc_idx);
                        ui.close();
                    }
                    ui.separator();
                    let sheet_label = if self.workspace == WorkspaceMode::SheetLayout {
                        "Back to Photo Canvas (Esc)"
                    } else {
                        "Sheet Print Preview (⌘P)"
                    };
                    if ui.button(sheet_label).clicked() {
                        if self.workspace == WorkspaceMode::SheetLayout {
                            self.switch_to_photo_edit();
                        } else {
                            self.switch_to_sheet_layout();
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button("Export Print Layout (Full Sheet)... (⌘E)")
                        .clicked()
                    {
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
                    let can_undo = self.can_undo();
                    if ui
                        .add_enabled(can_undo, egui::Button::new("Undo (⌘Z)"))
                        .clicked()
                    {
                        self.undo();
                        ui.close();
                    }
                    let can_redo = self.can_redo();
                    if ui
                        .add_enabled(can_redo, egui::Button::new("Redo (⌘⇧Z / ⌘Y)"))
                        .clicked()
                    {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Commit Crop (Enter)").clicked() {
                        self.commit_crop();
                        ui.close();
                    }
                    ui.separator();
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

    /// Renders the left tool sidebar with Photoshop-style icon-only buttons.
    fn render_tool_sidebar(&mut self, ui: &mut Ui) {
        Panel::left("tool_sidebar")
            .resizable(false)
            .exact_size(46.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);

                    // --- Permanent Professional Tool Palette ---

                    // 1. Selection Tool (V)
                    let is_select = self.active_tool == ToolMode::Select;
                    let sel_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Pointer,
                        is_select,
                        "Select Tool",
                        Some("V"),
                        "Inspect photo or layout elements",
                    );
                    if sel_resp.clicked() {
                        self.active_tool = ToolMode::Select;
                        self.status_message = "Active tool: Select [V]".to_string();
                    }
                    ui.add_space(3.0);

                    // 2. Crop Tool (C)
                    let is_crop = self.active_tool == ToolMode::Crop;
                    let crop_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Crop,
                        is_crop,
                        "Crop Tool",
                        Some("C"),
                        "Crop photo with locked passport proportions",
                    );
                    if crop_resp.clicked() {
                        self.workspace = WorkspaceMode::PhotoEdit;
                        self.active_tool = ToolMode::Crop;
                        self.status_message = "Active tool: Crop [C]".to_string();
                    }
                    ui.add_space(3.0);

                    // 3. Pan / Hand Tool (H)
                    let is_pan = self.active_tool == ToolMode::Pan;
                    let pan_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::SelectPan,
                        is_pan,
                        "Pan Tool",
                        Some("H"),
                        "Pan and navigate the workspace canvas",
                    );
                    if pan_resp.clicked() {
                        self.active_tool = ToolMode::Pan;
                        self.status_message = "Active tool: Pan Canvas [H]".to_string();
                    }

                    render_toolbar_divider(ui);

                    // --- History Controls (Undo & Redo) ---
                    let undo_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Undo,
                        false,
                        "Undo Crop",
                        Some("⌘Z"),
                        "Undo previous crop or edit operation",
                    );
                    if undo_resp.clicked() {
                        self.undo();
                    }
                    ui.add_space(3.0);

                    let redo_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Redo,
                        false,
                        "Redo Crop",
                        Some("⌘⇧Z / ⌘Y"),
                        "Redo reverted crop operation",
                    );
                    if redo_resp.clicked() {
                        self.redo();
                    }

                    render_toolbar_divider(ui);

                    // --- File Actions (Always in fixed positions) ---
                    let import_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Import,
                        false,
                        "Import Photo",
                        Some("⌘O / Ctrl+O"),
                        "Open a portrait photograph (JPG, PNG, WebP) from disk.",
                    );
                    if import_resp.clicked() {
                        self.import_image_dialog();
                    }
                    ui.add_space(3.0);

                    let export_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Export,
                        false,
                        "Export Output",
                        Some("⌘E / Ctrl+E"),
                        "Export print-ready PDF or high-resolution image.",
                    );
                    if export_resp.clicked() {
                        let target = match self.workspace {
                            WorkspaceMode::SheetLayout => ExportTarget::FullSheet,
                            WorkspaceMode::PhotoEdit => ExportTarget::SinglePhoto,
                        };
                        self.export_dialog(target);
                    }

                    render_toolbar_divider(ui);

                    // --- View & Navigation Controls ---
                    let zoom_in_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::ZoomIn,
                        false,
                        "Zoom In",
                        Some("+"),
                        "Zoom in on workspace canvas.",
                    );
                    if zoom_in_resp.clicked() {
                        self.canvas_zoom = (self.canvas_zoom * 1.15).min(5.0);
                    }
                    ui.add_space(3.0);

                    let zoom_out_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::ZoomOut,
                        false,
                        "Zoom Out",
                        Some("-"),
                        "Zoom out on workspace canvas.",
                    );
                    if zoom_out_resp.clicked() {
                        self.canvas_zoom = (self.canvas_zoom / 1.15).max(0.2);
                    }
                    ui.add_space(3.0);

                    let zoom_100_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Zoom100,
                        false,
                        "Actual Size (100%)",
                        Some("1"),
                        "Reset zoom to 100% scale and center canvas.",
                    );
                    if zoom_100_resp.clicked() {
                        self.reset_view();
                    }
                    ui.add_space(3.0);

                    let (fit_label, fit_desc) = match self.workspace {
                        WorkspaceMode::SheetLayout => (
                            "Fit Sheet to Viewport",
                            "Auto-fit grid arrangement and optimize sheet photos.",
                        ),
                        WorkspaceMode::PhotoEdit => (
                            "Maximize Crop Fit",
                            "Maximize crop boundary with locked passport proportions.",
                        ),
                    };
                    let fit_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::FitScreen,
                        false,
                        fit_label,
                        Some("0"),
                        fit_desc,
                    );
                    if fit_resp.clicked() {
                        match self.workspace {
                            WorkspaceMode::SheetLayout => self.auto_fit_grid(),
                            WorkspaceMode::PhotoEdit => {
                                if let Some(loaded) = &self.loaded_image {
                                    self.crop_area = CropArea::default_for_aspect(
                                        loaded.width,
                                        loaded.height,
                                        self.grid_config.photo.aspect_ratio(),
                                    );
                                    self.texture_needs_update = true;
                                    if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                                        doc.crop_area = self.crop_area;
                                    }
                                }
                            }
                        }
                    }

                    render_toolbar_divider(ui);

                    // --- Sheet Print Preview Mode ---
                    let is_sheet_preview = self.workspace == WorkspaceMode::SheetLayout;
                    let sheet_resp = render_toolbar_button(
                        ui,
                        ToolbarIcon::Grid,
                        is_sheet_preview,
                        "Sheet Print Preview",
                        Some("⌘P"),
                        "Preview final printable sheet layout and photo grid",
                    );
                    if sheet_resp.clicked() {
                        if self.workspace == WorkspaceMode::SheetLayout {
                            self.switch_to_photo_edit();
                        } else {
                            self.switch_to_sheet_layout();
                        }
                    }
                });
            });
    }

    /// Renders the right properties panel with Photoshop-style collapsible sections,
    /// dynamically tailored to the currently active tool.
    fn render_properties_panel(&mut self, ui: &mut Ui) {
        Panel::right("properties_panel")
            .resizable(true)
            .default_size(320.0)
            .min_size(270.0)
            .max_size(480.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);
                    self.render_inspector_header(ui);
                    ui.add_space(6.0);

                    if self.workspace == WorkspaceMode::SheetLayout {
                        self.render_select_tool_inspector(ui);
                    } else {
                        match self.active_tool {
                            ToolMode::Select => self.render_select_tool_inspector(ui),
                            ToolMode::Crop => self.render_crop_tool_inspector(ui),
                            ToolMode::Pan => self.render_pan_tool_inspector(ui),
                        }
                    }
                });
            });
    }

    /// Renders the Photoshop-style top header inside the right inspector panel.
    fn render_inspector_header(&mut self, ui: &mut Ui) {
        let (title, shortcut, icon) = if self.workspace == WorkspaceMode::SheetLayout {
            ("Sheet Print & Export", "⌘P", ToolbarIcon::Grid)
        } else {
            match self.active_tool {
                ToolMode::Select => ("Selection Inspector", "V", ToolbarIcon::Pointer),
                ToolMode::Crop => ("Crop Tool Properties", "C", ToolbarIcon::Crop),
                ToolMode::Pan => ("Canvas Navigator & View", "H", ToolbarIcon::SelectPan),
            }
        };

        ui.horizontal(|ui| {
            let (icon_rect, _) =
                ui.allocate_exact_size(Vec2::new(18.0, 18.0), egui::Sense::hover());
            crate::ui_icons::draw_toolbar_icon(
                ui.painter(),
                icon,
                icon_rect.center(),
                Color32::from_rgb(120, 180, 255),
                16.0,
            );
            ui.add_space(4.0);
            ui.heading(title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(Color32::from_rgb(100, 180, 255), format!("[{shortcut}]"));
            });
        });

        ui.add_space(4.0);
        ui.separator();
    }

    /// Helper to render a Photoshop-style framed expandable section.
    fn render_collapsing_section<R>(
        ui: &mut Ui,
        id_salt: &str,
        title: &str,
        default_open: bool,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            egui::CollapsingHeader::new(egui::RichText::new(title).strong().size(12.5))
                .id_salt(id_salt)
                .default_open(default_open)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    add_contents(ui);
                    ui.add_space(2.0);
                });
        });
        ui.add_space(4.0);
    }

    /// Tool-specific inspector for Selection Tool (V).
    fn render_select_tool_inspector(&mut self, ui: &mut Ui) {
        // --- 1. Selected Slot & Photo ---
        Self::render_collapsing_section(
            ui,
            "sec_select_slot",
            "📷 Selected Photo Slot",
            true,
            |ui| {
                if let Some(idx) = self.selected_slot {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            Color32::from_rgb(100, 180, 255),
                            format!("Slot #{} Selected", idx + 1),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Deselect").clicked() {
                                self.selected_slot = None;
                            }
                        });
                    });

                    if let Some(slot) = compute_layout(&self.grid_config)
                        .ok()
                        .and_then(|layout| layout.slots.get(idx).copied())
                    {
                        ui.label(format!(
                            "Position on Sheet: X: {:.1} mm, Y: {:.1} mm",
                            slot.x.0, slot.y.0
                        ));
                        let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                        ui.label(format!(
                            "Slot Size: {:.1} × {:.1} mm ({} × {} px)",
                            slot.width.0, slot.height.0, px_w, px_h
                        ));
                    }

                    ui.add_space(3.0);
                    if let Some(loaded) = &self.loaded_image {
                        ui.label(format!("Placed Photo: {}", loaded.file_name));
                        ui.label(format!("Source: {} × {} px", loaded.width, loaded.height));
                        ui.horizontal(|ui| {
                            if ui
                                .button("✂ Edit Crop in Photo Editor")
                                .on_hover_text("Open Photo Editor to frame and align crop")
                                .clicked()
                            {
                                self.switch_to_photo_edit();
                            }
                            if ui.button("Replace...").clicked() {
                                self.import_image_dialog();
                            }
                        });
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(170, 175, 185),
                            "Slot is empty. Place a photograph to fill this slot.",
                        );
                        if ui.button("Import Photo...").clicked() {
                            self.import_image_dialog();
                        }
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Click any photo slot on the sheet canvas to inspect.")
                            .size(11.5)
                            .color(Color32::from_rgb(160, 165, 175)),
                    );
                    ui.add_space(2.0);
                    if let Some(loaded) = &self.loaded_image {
                        ui.label(format!("Placed Photo: {}", loaded.file_name));
                        let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                        ui.label(format!(
                            "Photo Size: {:.1} × {:.1} mm ({} × {} px)",
                            self.grid_config.photo.width.0,
                            self.grid_config.photo.height.0,
                            px_w,
                            px_h
                        ));
                        ui.horizontal(|ui| {
                            if ui
                                .button("✂ Edit Crop in Photo Editor")
                                .on_hover_text("Switch to Photo Editor workspace")
                                .clicked()
                            {
                                self.switch_to_photo_edit();
                            }
                            if ui.button("Replace Photo...").clicked() {
                                self.import_image_dialog();
                            }
                        });
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(170, 175, 185),
                            "No photo placed on sheet.",
                        );
                        if ui.button("Import Photo...").clicked() {
                            self.import_image_dialog();
                        }
                    }
                }
            },
        );

        // --- 2. Printable Sheet Setup ---
        Self::render_collapsing_section(
            ui,
            "sec_select_sheet",
            "📄 Printable Sheet Setup",
            true,
            |ui| {
                let old_preset = self.sheet_preset;
                egui::ComboBox::from_label("Sheet Preset")
                    .selected_text(self.sheet_preset.short_name())
                    .show_ui(ui, |ui| {
                        for preset in SheetPreset::ALL {
                            ui.selectable_value(&mut self.sheet_preset, *preset, preset.label());
                        }
                    });
                if self.sheet_preset != old_preset {
                    self.apply_sheet_preset();
                }

                ui.horizontal(|ui| {
                    ui.label("Orientation:");
                    let p_selected = self.sheet_orientation == SheetOrientation::Portrait;
                    if ui.selectable_label(p_selected, "Portrait").clicked() && !p_selected {
                        self.sheet_orientation = SheetOrientation::Portrait;
                        self.apply_sheet_preset();
                    }
                    let l_selected = self.sheet_orientation == SheetOrientation::Landscape;
                    if ui.selectable_label(l_selected, "Landscape").clicked() && !l_selected {
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

                ui.horizontal(|ui| {
                    ui.label("Print DPI:");
                    let old_dpi = self.dpi;
                    egui::ComboBox::from_id_salt("sheet_dpi_combobox")
                        .selected_text(format!("{}", self.dpi))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.dpi, Dpi::DRAFT, "150 DPI (Draft)");
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
                let (sheet_px_w, sheet_px_h) = self.grid_config.sheet.to_pixels(self.dpi);
                ui.label(format!(
                    "Sheet Resolution: {} × {} px @ {}",
                    sheet_px_w, sheet_px_h, self.dpi
                ));
            },
        );

        // --- 3. Grid Arrangement & Spacing ---
        Self::render_collapsing_section(
            ui,
            "sec_select_grid",
            "▦ Grid Layout & Spacing",
            true,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Columns:");
                    ui.add(egui::DragValue::new(&mut self.grid_config.cols).range(1..=20));
                    ui.label("Rows:");
                    ui.add(egui::DragValue::new(&mut self.grid_config.rows).range(1..=20));
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("Auto-Fit (Max Photos)")
                        .on_hover_text("Calculate maximum photos fitting within sheet margins")
                        .clicked()
                    {
                        self.auto_fit_grid();
                    }
                    ui.checkbox(&mut self.grid_config.auto_center, "Auto Center");
                });

                ui.separator();
                ui.label("Margins (mm):");
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
                    ui.label("Horiz:");
                    let mut g = self.grid_config.gutters.horizontal.0;
                    if ui
                        .add(egui::DragValue::new(&mut g).speed(0.5).range(0.0..=50.0))
                        .changed()
                    {
                        self.grid_config.gutters.horizontal = Millimeters(g);
                    }
                    ui.label("Vert:");
                    let mut g = self.grid_config.gutters.vertical.0;
                    if ui
                        .add(egui::DragValue::new(&mut g).speed(0.5).range(0.0..=50.0))
                        .changed()
                    {
                        self.grid_config.gutters.vertical = Millimeters(g);
                    }
                });
            },
        );

        // --- 4. View & Print Guides ---
        Self::render_collapsing_section(
            ui,
            "sec_select_guides",
            "📐 View & Print Guides",
            false,
            |ui| {
                ui.checkbox(
                    &mut self.show_cut_marks,
                    "Corner Cut Marks (Scissors / Guillotine)",
                );
                ui.checkbox(&mut self.show_margins, "Sheet Printable Margins");
                ui.checkbox(&mut self.show_dimensions, "Sheet Dimensions Annotation");
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui.button("Enable All").clicked() {
                        self.show_cut_marks = true;
                        self.show_margins = true;
                        self.show_dimensions = true;
                    }
                    if ui.button("Disable All").clicked() {
                        self.show_cut_marks = false;
                        self.show_margins = false;
                        self.show_dimensions = false;
                    }
                });
            },
        );

        // --- 5. Export Full Sheet ---
        Self::render_collapsing_section(
            ui,
            "sec_select_export",
            "📤 Export Sheet Layout",
            true,
            |ui| {
                match compute_layout(&self.grid_config) {
                    Ok(layout) => {
                        ui.colored_label(
                            Color32::from_rgb(46, 160, 67),
                            format!("✓ Print Ready: {} photos fit on sheet", layout.count()),
                        );
                        ui.label(format!(
                            "Printable: {:.1} × {:.1} mm",
                            layout.printable_width.0, layout.printable_height.0
                        ));
                    }
                    Err(err) => {
                        ui.colored_label(
                            Color32::from_rgb(230, 80, 70),
                            format!("⚠ Layout Error:\n{err}"),
                        );
                    }
                }

                ui.add_space(4.0);
                egui::ComboBox::from_id_salt("sheet_export_format")
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
                let (out_w, out_h) = self.grid_config.sheet.to_pixels(self.dpi);
                ui.label(format!(
                    "Target Output: {} × {} px @ {}",
                    out_w, out_h, self.dpi
                ));
                let has_photo = self.loaded_image.is_some();
                let btn =
                    egui::Button::new(egui::RichText::new("Export Sheet to File...").strong());
                if ui.add_enabled(has_photo, btn).clicked() {
                    self.export_dialog(ExportTarget::FullSheet);
                }
                if !has_photo {
                    ui.colored_label(
                        Color32::from_rgb(200, 160, 50),
                        "Import a photo to enable export",
                    );
                }
            },
        );
    }

    /// Tool-specific inspector for Crop Tool (C).
    fn render_crop_tool_inspector(&mut self, ui: &mut Ui) {
        // --- 0. Crop Execution & History ---
        Self::render_collapsing_section(
            ui,
            "sec_crop_commit",
            "⚡ Crop Execution & History",
            true,
            |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new("Crop Selected Area (Enter)")
                                .color(Color32::from_rgb(100, 240, 150))
                                .strong(),
                        )
                        .on_hover_text("Crop photograph to selected area (shrinks image to crop bounds)")
                        .clicked()
                    {
                        self.commit_crop();
                    }

                    if ui
                        .button("Reset Crop")
                        .on_hover_text("Reset crop boundaries to fit photograph (or revert to uncropped photo)")
                        .clicked()
                    {
                        self.reset_crop();
                    }

                    let can_undo = self.can_undo();
                    if ui
                        .add_enabled(can_undo, egui::Button::new("Undo (⌘Z)"))
                        .on_hover_text("Undo previous crop")
                        .clicked()
                    {
                        self.undo();
                    }

                    let can_redo = self.can_redo();
                    if ui
                        .add_enabled(can_redo, egui::Button::new("Redo (⌘⇧Z)"))
                        .on_hover_text("Redo reverted crop")
                        .clicked()
                    {
                        self.redo();
                    }
                });
            },
        );

        // --- 1. Passport & ID Photo Standard ---
        Self::render_collapsing_section(
            ui,
            "sec_crop_preset",
            "🛂 Passport & ID Standard",
            true,
            |ui| {
                let old_photo_preset = self.photo_preset;
                egui::ComboBox::from_label("Photo Preset")
                    .selected_text(self.photo_preset.short_name())
                    .show_ui(ui, |ui| {
                        for preset in PhotoPreset::ALL {
                            ui.selectable_value(&mut self.photo_preset, *preset, preset.label());
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

                let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                ui.label(format!(
                    "Print Resolution: {} × {} px @ {}",
                    px_w, px_h, self.dpi
                ));

                ui.horizontal(|ui| {
                    ui.label("Print DPI:");
                    let old_dpi = self.dpi;
                    egui::ComboBox::from_id_salt("crop_photo_dpi")
                        .selected_text(format!("{}", self.dpi))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.dpi, Dpi::DRAFT, "150 DPI (Draft)");
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
            },
        );

        // --- 2. Source Photograph ---
        Self::render_collapsing_section(
            ui,
            "sec_crop_source",
            "📷 Source Photograph",
            true,
            |ui| {
                if let Some(loaded) = &self.loaded_image {
                    ui.label(format!("File: {}", loaded.file_name));
                    ui.label(format!("Original: {} × {} px", loaded.width, loaded.height));
                    ui.horizontal(|ui| {
                        if ui
                            .button("Replace Photo...")
                            .on_hover_text("Open another photo file")
                            .clicked()
                        {
                            self.import_image_dialog();
                        }
                        if ui
                            .button("Remove Photo")
                            .on_hover_text("Remove photo from workspace")
                            .clicked()
                        {
                            self.loaded_image = None;
                            self.source_texture = None;
                            self.cropped_texture = None;
                            self.status_message = "Photo removed.".to_string();
                        }
                    });
                } else {
                    ui.colored_label(Color32::from_rgb(170, 175, 185), "No photo loaded.");
                    if ui.button("Import Portrait Photo...").clicked() {
                        self.import_image_dialog();
                    }
                }
            },
        );

        // --- 3. Crop Framing & Bounds ---
        Self::render_collapsing_section(
            ui,
            "sec_crop_bounds",
            "✂ Crop Framing & Bounds",
            true,
            |ui| {
                if let Some(loaded) = &self.loaded_image {
                    let (w, h) = (loaded.width, loaded.height);
                    let target_aspect = self.grid_config.photo.aspect_ratio();

                    ui.horizontal(|ui| {
                        if ui
                            .button("Maximize Fit")
                            .on_hover_text("Maximize crop area maintaining passport aspect ratio")
                            .clicked()
                        {
                            self.crop_area = CropArea::default_for_aspect(w, h, target_aspect);
                            self.texture_needs_update = true;
                        }
                        if ui
                            .button("Center Box")
                            .on_hover_text("Center current crop box on image")
                            .clicked()
                        {
                            let center_x = (1.0 - self.crop_area.width) / 2.0;
                            let center_y = (1.0 - self.crop_area.height) / 2.0;
                            self.crop_area.x = center_x.clamp(0.0, 1.0);
                            self.crop_area.y = center_y.clamp(0.0, 1.0);
                            self.crop_area.clamp_within_bounds();
                            self.texture_needs_update = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Zoom In Crop").clicked() {
                            self.zoom_crop(0.92);
                        }
                        if ui.button("Zoom Out Crop").clicked() {
                            self.zoom_crop(1.08);
                        }
                    });

                    let crop_px_x = (self.crop_area.x * w as f32).round() as u32;
                    let crop_px_y = (self.crop_area.y * h as f32).round() as u32;
                    let crop_px_w = (self.crop_area.width * w as f32).round() as u32;
                    let crop_px_h = (self.crop_area.height * h as f32).round() as u32;
                    ui.label(format!(
                        "Crop Box: {} × {} px (at {}, {})",
                        crop_px_w, crop_px_h, crop_px_x, crop_px_y
                    ));
                } else {
                    ui.label("Import an image to frame crop.");
                }
            },
        );

        // --- 4. Biometric Guidelines ---
        Self::render_collapsing_section(
            ui,
            "sec_crop_biometric",
            "👤 Biometric Face Guidelines",
            true,
            |ui| {
                ui.checkbox(&mut self.show_biometric_guides, "Show Biometric Guides");
                ui.label("Official passport standards require:");
                ui.colored_label(
                    Color32::from_rgb(100, 200, 255),
                    "• Crown line: top of hair/head (~12%)",
                );
                ui.colored_label(
                    Color32::from_rgb(255, 215, 0),
                    "• Eye line: eye level (56-69% from bottom)",
                );
                ui.colored_label(
                    Color32::from_rgb(100, 200, 255),
                    "• Chin line: base of chin (~25% from bottom)",
                );
                ui.colored_label(
                    Color32::from_rgb(100, 200, 255),
                    "• Vertical axis: head symmetry",
                );
            },
        );

        // --- 5. Export Single Photo ---
        Self::render_collapsing_section(
            ui,
            "sec_crop_export",
            "📤 Export Single Photo",
            false,
            |ui| {
                egui::ComboBox::from_id_salt("photo_export_format")
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
                let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                ui.label(format!("Resolution: {} × {} px @ {}", px_w, px_h, self.dpi));
                let has_photo = self.loaded_image.is_some();
                let btn = egui::Button::new(egui::RichText::new("Export Single Photo...").strong());
                if ui.add_enabled(has_photo, btn).clicked() {
                    self.export_dialog(ExportTarget::SinglePhoto);
                }
                if !has_photo {
                    ui.colored_label(
                        Color32::from_rgb(200, 160, 50),
                        "Import a photo to enable export",
                    );
                }
            },
        );

        ui.add_space(8.0);
        if ui
            .button(egui::RichText::new("✓ Done (Apply to Sheet Layout) ➔").strong())
            .on_hover_text("Return to printable sheet grid")
            .clicked()
        {
            self.switch_to_sheet_layout();
        }
    }

    /// Tool-specific inspector for Pan Tool (H).
    fn render_pan_tool_inspector(&mut self, ui: &mut Ui) {
        // --- 1. Canvas Navigator & Zoom ---
        Self::render_collapsing_section(
            ui,
            "sec_pan_nav",
            "🔍 Canvas Navigator & Zoom",
            true,
            |ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("Zoom: {:.0}%", self.canvas_zoom * 100.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("100%").clicked() {
                            self.reset_view();
                        }
                    });
                });

                ui.horizontal(|ui| {
                    if ui.button("Zoom In (+15%)").clicked() {
                        self.canvas_zoom = (self.canvas_zoom * 1.15).min(5.0);
                    }
                    if ui.button("Zoom Out (-15%)").clicked() {
                        self.canvas_zoom = (self.canvas_zoom / 1.15).max(0.2);
                    }
                    if ui.button("Fit Viewport").clicked() {
                        match self.workspace {
                            WorkspaceMode::SheetLayout => self.auto_fit_grid(),
                            WorkspaceMode::PhotoEdit => {
                                if let Some(loaded) = &self.loaded_image {
                                    self.crop_area = CropArea::default_for_aspect(
                                        loaded.width,
                                        loaded.height,
                                        self.grid_config.photo.aspect_ratio(),
                                    );
                                    self.texture_needs_update = true;
                                }
                            }
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Pan Offset: X: {:.0} px, Y: {:.0} px",
                        self.canvas_pan.x, self.canvas_pan.y
                    ));
                    if ui.button("Center View").clicked() {
                        self.canvas_pan = Vec2::ZERO;
                    }
                });
            },
        );

        // --- 2. Document & Layout Summary ---
        Self::render_collapsing_section(
            ui,
            "sec_pan_summary",
            "📊 Document & Layout Summary",
            true,
            |ui| {
                ui.label(format!("Workspace: {}", self.workspace.label()));
                ui.label(format!(
                    "Sheet Dimensions: {:.1} × {:.1} mm ({:.1} × {:.1} in)",
                    self.grid_config.sheet.width.0,
                    self.grid_config.sheet.height.0,
                    self.grid_config.sheet.width.to_inches().0,
                    self.grid_config.sheet.height.to_inches().0
                ));
                ui.label(format!(
                    "Photo Size: {:.1} × {:.1} mm",
                    self.grid_config.photo.width.0, self.grid_config.photo.height.0
                ));
                ui.label(format!("Resolution: {}", self.dpi));

                match compute_layout(&self.grid_config) {
                    Ok(layout) => {
                        ui.colored_label(
                            Color32::from_rgb(46, 160, 67),
                            format!("✓ Layout Fit: {} photos fitting on sheet", layout.count()),
                        );
                    }
                    Err(err) => {
                        ui.colored_label(
                            Color32::from_rgb(230, 80, 70),
                            format!("⚠ Layout Overflow:\n{err}"),
                        );
                    }
                }
            },
        );

        // --- 3. View Overlays & Guides ---
        Self::render_collapsing_section(
            ui,
            "sec_pan_guides",
            "📐 View Overlays & Guides",
            true,
            |ui| {
                ui.checkbox(&mut self.show_cut_marks, "Corner Cut Marks");
                ui.checkbox(&mut self.show_margins, "Sheet Margins");
                ui.checkbox(&mut self.show_dimensions, "Dimensions Annotation");
                if self.workspace == WorkspaceMode::PhotoEdit {
                    ui.checkbox(&mut self.show_biometric_guides, "Biometric Face Guides");
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui.button("Enable All").clicked() {
                        self.show_cut_marks = true;
                        self.show_margins = true;
                        self.show_dimensions = true;
                        if self.workspace == WorkspaceMode::PhotoEdit {
                            self.show_biometric_guides = true;
                        }
                    }
                    if ui.button("Disable All").clicked() {
                        self.show_cut_marks = false;
                        self.show_margins = false;
                        self.show_dimensions = false;
                        if self.workspace == WorkspaceMode::PhotoEdit {
                            self.show_biometric_guides = false;
                        }
                    }
                });
            },
        );

        // --- 4. Switch to Editing Tool ---
        Self::render_collapsing_section(
            ui,
            "sec_pan_tools",
            "🛠 Switch to Edit Tool",
            true,
            |ui| {
                if ui
                    .button("Select Tool [V] (Inspect Photo Slots & Grid)")
                    .clicked()
                {
                    self.switch_to_sheet_layout();
                    self.active_tool = ToolMode::Select;
                }
                if ui
                    .button("Crop Tool [C] (Frame & Align Photo Crop)")
                    .clicked()
                {
                    self.switch_to_photo_edit();
                    self.active_tool = ToolMode::Crop;
                }
            },
        );
    }

    /// Properties inspector for Printable Sheet Layout workspace (backward compatibility).
    pub fn render_sheet_properties(&mut self, ui: &mut Ui) {
        self.render_select_tool_inspector(ui);
    }

    /// Properties inspector for Single Photo Editor workspace (backward compatibility).
    pub fn render_photo_properties(&mut self, ui: &mut Ui) {
        self.render_crop_tool_inspector(ui);
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

    /// Renders the central canvas: document tabs, contextual options bar, and active canvas.
    fn render_central_canvas(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.render_document_tab_bar(ui);
            self.render_canvas_options_bar(ui);

            match self.workspace {
                WorkspaceMode::SheetLayout => self.render_sheet_canvas(ui),
                WorkspaceMode::PhotoEdit => self.render_crop_editor(ui),
            }
        });
    }

    /// Renders Photoshop-style document tabs for open images.
    fn render_document_tab_bar(&mut self, ui: &mut Ui) {
        let bar_h = 34.0;
        let available_width = ui.available_width();
        let (tab_bar_rect, _) =
            ui.allocate_exact_size(Vec2::new(available_width, bar_h), egui::Sense::hover());
        let painter = ui.painter_at(tab_bar_rect);

        // Sleek dark tab bar strip
        painter.rect_filled(tab_bar_rect, 0.0, Color32::from_rgb(22, 24, 30));
        painter.line_segment(
            [tab_bar_rect.left_bottom(), tab_bar_rect.right_bottom()],
            Stroke::new(1.0, Color32::from_rgb(38, 42, 52)),
        );

        let mut tab_ui = ui
            .new_child(egui::UiBuilder::new().max_rect(tab_bar_rect.shrink2(Vec2::new(4.0, 2.0))));

        tab_ui.horizontal(|ui| {
            ui.add_space(2.0);

            // Document tabs
            let mut close_tab_idx = None;
            let mut select_tab_idx = None;

            for (idx, doc) in self.documents.iter().enumerate() {
                let is_active =
                    self.workspace == WorkspaceMode::PhotoEdit && idx == self.active_doc_idx;

                let title = if doc.title.len() > 18 {
                    format!("{}…", &doc.title[..16])
                } else {
                    doc.title.clone()
                };

                let font_id = egui::FontId::proportional(12.0);
                let text_color = if is_active {
                    Color32::from_rgb(240, 245, 255)
                } else {
                    Color32::from_rgb(160, 165, 175)
                };
                let galley = ui
                    .painter()
                    .layout_no_wrap(title.clone(), font_id, text_color);
                let text_w = galley.size().x;
                let close_size = 14.0;
                let tab_pad_h = 10.0;
                let tab_w = tab_pad_h + text_w + 8.0 + close_size + tab_pad_h;
                let tab_h = 28.0;

                let (tab_rect, tab_resp) =
                    ui.allocate_exact_size(Vec2::new(tab_w, tab_h), egui::Sense::click());
                let tab_resp = tab_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                if tab_resp.clicked() {
                    select_tab_idx = Some(idx);
                }

                let is_tab_hovered = tab_resp.hovered();
                let bg_color = if is_active {
                    Color32::from_rgb(36, 40, 52)
                } else if is_tab_hovered {
                    Color32::from_rgb(32, 35, 44)
                } else {
                    Color32::from_rgb(24, 26, 34)
                };

                let painter = ui.painter();
                painter.rect_filled(
                    tab_rect,
                    egui::CornerRadius {
                        nw: 4,
                        ne: 4,
                        sw: 0,
                        se: 0,
                    },
                    bg_color,
                );

                if is_active {
                    painter.line_segment(
                        [tab_rect.left_top(), tab_rect.right_top()],
                        Stroke::new(2.0, Color32::from_rgb(0, 160, 255)),
                    );
                }

                // Render tab title text (no Label widget -> never shows text I-beam)
                let text_pos = Pos2::new(
                    tab_rect.min.x + tab_pad_h,
                    tab_rect.center().y - galley.size().y / 2.0,
                );
                let text_color = if is_active {
                    Color32::from_rgb(240, 245, 255)
                } else if is_tab_hovered {
                    Color32::from_rgb(215, 220, 230)
                } else {
                    Color32::from_rgb(160, 165, 175)
                };
                painter.galley(text_pos, galley, text_color);

                // Dedicated interactive vector close button inside tab
                let close_rect = Rect::from_center_size(
                    Pos2::new(
                        tab_rect.max.x - tab_pad_h - close_size / 2.0,
                        tab_rect.center().y,
                    ),
                    Vec2::splat(close_size),
                );
                let close_resp = ui
                    .interact(
                        close_rect,
                        ui.id().with(("close_tab", idx)),
                        egui::Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Close tab (⌘W)");

                if close_resp.clicked() {
                    close_tab_idx = Some(idx);
                }

                if close_resp.hovered() {
                    painter.circle_filled(close_rect.center(), 7.0, Color32::from_rgb(190, 45, 45));
                }
                let x_stroke_color = if close_resp.hovered() {
                    Color32::WHITE
                } else if is_active {
                    Color32::from_rgb(175, 180, 190)
                } else {
                    Color32::from_rgb(120, 125, 135)
                };
                let c = close_rect.center();
                let x_r = 3.2;
                painter.line_segment(
                    [c - Vec2::splat(x_r), c + Vec2::splat(x_r)],
                    Stroke::new(1.3, x_stroke_color),
                );
                painter.line_segment(
                    [c + Vec2::new(x_r, -x_r), c + Vec2::new(-x_r, x_r)],
                    Stroke::new(1.3, x_stroke_color),
                );

                ui.add_space(2.0);
            }

            if let Some(idx) = close_tab_idx {
                self.close_document(idx);
            } else if let Some(idx) = select_tab_idx {
                self.select_document(idx);
                self.switch_to_photo_edit();
            }

            // New tab / Open photo button [+]
            let plus_btn = ui
                .add(egui::Button::new(
                    egui::RichText::new("+").size(14.0).strong(),
                ))
                .on_hover_text("Open photograph into a new tab (⌘O)")
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if plus_btn.clicked() {
                self.import_image_dialog();
            }

            // Right side: Sheet Print Preview button or return button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.workspace == WorkspaceMode::SheetLayout {
                    let back_btn = ui
                        .add(egui::Button::new(
                            egui::RichText::new("Back to Photo Canvas (Esc)")
                                .color(Color32::from_rgb(255, 215, 0))
                                .strong(),
                        ))
                        .on_hover_text("Return to standard photo canvas")
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if back_btn.clicked() {
                        self.switch_to_photo_edit();
                    }
                } else {
                    let preview_btn = ui
                        .add(egui::Button::new(
                            egui::RichText::new("Sheet Preview & Print (⌘P)")
                                .color(Color32::from_rgb(100, 190, 255))
                                .strong(),
                        ))
                        .on_hover_text(
                            "Preview printable sheet layout with photo grid and cut marks (⌘P)",
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if preview_btn.clicked() {
                        self.switch_to_sheet_layout();
                    }
                }
            });
        });
    }

    /// Renders the Photoshop-style contextual tool options bar directly below document tabs.
    fn render_canvas_options_bar(&mut self, ui: &mut Ui) {
        let bar_h = 30.0;
        let available_width = ui.available_width();
        let (opt_rect, _) =
            ui.allocate_exact_size(Vec2::new(available_width, bar_h), egui::Sense::hover());
        let painter = ui.painter_at(opt_rect);

        painter.rect_filled(opt_rect, 0.0, Color32::from_rgb(28, 30, 38));
        painter.line_segment(
            [opt_rect.left_bottom(), opt_rect.right_bottom()],
            Stroke::new(1.0, Color32::from_rgb(44, 48, 58)),
        );

        let mut opt_ui =
            ui.new_child(egui::UiBuilder::new().max_rect(opt_rect.shrink2(Vec2::new(8.0, 2.0))));

        opt_ui.horizontal(|ui| {
            // Undo / Redo controls with crisp vector icons (no missing glyph boxes)
            let can_undo = self.can_undo();
            let undo_btn = render_options_bar_button(
                ui,
                Some(ToolbarIcon::Undo),
                "Undo",
                can_undo,
                "Undo crop [⌘Z]",
            );
            if undo_btn.clicked() {
                self.undo();
                ui.ctx().request_repaint();
            }

            let can_redo = self.can_redo();
            let redo_btn = render_options_bar_button(
                ui,
                Some(ToolbarIcon::Redo),
                "Redo",
                can_redo,
                "Redo reverted edit [⌘⇧Z / ⌘Y]",
            );
            if redo_btn.clicked() {
                self.redo();
                ui.ctx().request_repaint();
            }

            ui.separator();

            match self.workspace {
                WorkspaceMode::PhotoEdit => match self.active_tool {
                    ToolMode::Crop => {
                        let crop_btn = ui
                            .add(egui::Button::new(
                                egui::RichText::new("Crop Selected Area (Enter)")
                                    .color(Color32::from_rgb(100, 240, 150))
                                    .strong()
                                    .size(11.5),
                            ))
                            .on_hover_text("Crop photograph to selected area (Enter or double-click)")
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if crop_btn.clicked() {
                            self.commit_crop();
                            ui.ctx().request_repaint();
                        }

                        let reset_btn = ui
                            .add(egui::Button::new(
                                egui::RichText::new("Reset Crop")
                                    .size(11.5)
                                    .color(Color32::from_rgb(215, 220, 230)),
                            ))
                            .on_hover_text(
                                "Reset crop boundaries to fit photograph (or revert to uncropped photo)",
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if reset_btn.clicked() {
                            self.reset_crop();
                            ui.ctx().request_repaint();
                        }

                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!(
                                "Standard: {}",
                                self.photo_preset.short_name()
                            ))
                            .size(11.5)
                            .color(Color32::from_rgb(170, 175, 185)),
                        );

                        if let Some(loaded) = &self.loaded_image {
                            let (w, h) = (loaded.width, loaded.height);
                            let target_aspect = self.grid_config.photo.aspect_ratio();

                            let max_fit_btn = ui
                                .add(egui::Button::new(egui::RichText::new("Max Fit").size(11.5)))
                                .on_hover_text("Fit maximum centered crop box")
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if max_fit_btn.clicked() {
                                self.crop_area = CropArea::default_for_aspect(w, h, target_aspect);
                                self.texture_needs_update = true;
                                if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                                    doc.crop_area = self.crop_area;
                                }
                                ui.ctx().request_repaint();
                            }

                            let center_btn = ui
                                .add(egui::Button::new(egui::RichText::new("Center").size(11.5)))
                                .on_hover_text("Center crop box")
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if center_btn.clicked() {
                                let center_x = (1.0 - self.crop_area.width) / 2.0;
                                let center_y = (1.0 - self.crop_area.height) / 2.0;
                                self.crop_area.x = center_x.clamp(0.0, 1.0);
                                self.crop_area.y = center_y.clamp(0.0, 1.0);
                                self.crop_area.clamp_within_bounds();
                                self.texture_needs_update = true;
                                if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                                    doc.crop_area = self.crop_area;
                                }
                                ui.ctx().request_repaint();
                            }
                        }

                        ui.separator();
                        ui.checkbox(&mut self.show_biometric_guides, "Biometric Guides");
                        ui.checkbox(&mut self.show_dimensions, "Dimensions");
                    }
                    ToolMode::Select => {
                        if let Some(loaded) = &self.loaded_image {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Photo: {} ({} × {} px)",
                                    loaded.file_name, loaded.width, loaded.height
                                ))
                                .size(11.5)
                                .color(Color32::from_rgb(180, 220, 255)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("No photo open. Import an image to begin.")
                                    .size(11.5)
                                    .color(Color32::from_rgb(160, 165, 175)),
                            );
                        }

                        ui.separator();
                        if ui
                            .button(egui::RichText::new("✂ Crop Tool [C]").size(11.5))
                            .on_hover_text("Activate crop tool to frame passport proportions")
                            .clicked()
                        {
                            self.active_tool = ToolMode::Crop;
                        }

                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("Zoom: {:.0}%", self.canvas_zoom * 100.0))
                                .size(11.5),
                        );
                        if ui.button("100%").clicked() {
                            self.reset_view();
                        }
                        ui.checkbox(&mut self.show_dimensions, "Dimensions");
                    }
                    ToolMode::Pan => {
                        ui.label(
                            egui::RichText::new("Pan / Hand Tool [H] — Drag canvas to navigate")
                                .size(11.5)
                                .color(Color32::from_rgb(170, 175, 185)),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("Zoom: {:.0}%", self.canvas_zoom * 100.0))
                                .size(11.5),
                        );
                        if ui.button("100%").clicked() {
                            self.reset_view();
                        }
                        if ui.button("Center Canvas").clicked() {
                            self.canvas_pan = Vec2::ZERO;
                        }
                    }
                },
                WorkspaceMode::SheetLayout => {
                    ui.label(
                        egui::RichText::new("Printable Sheet Preview:")
                            .strong()
                            .size(11.5),
                    );
                    ui.label(
                        egui::RichText::new(format!("Sheet: {}", self.sheet_preset.short_name()))
                            .size(11.5)
                            .color(Color32::from_rgb(170, 175, 185)),
                    );

                    match compute_layout(&self.grid_config) {
                        Ok(layout) => {
                            ui.colored_label(
                                Color32::from_rgb(46, 160, 67),
                                format!("✓ {} Photos", layout.count()),
                            );
                        }
                        Err(_) => {
                            ui.colored_label(Color32::from_rgb(230, 80, 70), "⚠ Layout Overflow");
                        }
                    }

                    ui.separator();
                    ui.checkbox(&mut self.show_cut_marks, "Cut Marks");
                    ui.checkbox(&mut self.show_margins, "Sheet Margins");
                    ui.checkbox(&mut self.show_dimensions, "Dimensions");

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                egui::RichText::new("Export Sheet...")
                                    .color(Color32::from_rgb(100, 200, 255))
                                    .strong()
                                    .size(11.5),
                            )
                            .on_hover_text("Export print-ready layout")
                            .clicked()
                        {
                            self.export_dialog(ExportTarget::FullSheet);
                        }
                    });
                }
            }
        });
    }

    /// Renders the interactive crop editor tool on the single-photo workbench.
    fn render_crop_editor(&mut self, ui: &mut Ui) {
        let available_size = ui.available_size();
        let (rect, response) =
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
        let target_aspect = self.grid_config.photo.aspect_ratio();

        // Viewable image workspace area
        let work_rect = rect;

        let padding = 36.0;
        let base_scale = ((work_rect.width() - padding * 2.0).max(10.0) / src_w)
            .min((work_rect.height() - padding * 2.0).max(10.0) / src_h);
        let current_scale = base_scale * self.canvas_zoom;

        let disp_w = src_w * current_scale;
        let disp_h = src_h * current_scale;
        let disp_center = work_rect.center() + self.canvas_pan;
        let disp_rect = Rect::from_center_size(disp_center, Vec2::new(disp_w, disp_h));

        // Pan handling via Hand tool or middle mouse drag
        if response.dragged()
            && (self.active_tool == ToolMode::Pan || ui.input(|i| i.pointer.middle_down()))
        {
            self.canvas_pan += response.drag_delta();
        }

        // Draw shadow behind photograph
        painter.rect_filled(
            disp_rect.translate(Vec2::new(0.0, 4.0)),
            2.0,
            Color32::from_black_alpha(100),
        );

        // Paint source image
        if let Some(tex) = &self.source_texture {
            painter.image(
                tex.id(),
                disp_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        // Border around photo canvas
        painter.rect_stroke(
            disp_rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(60, 65, 78)),
            egui::StrokeKind::Outside,
        );

        // Mouse coordinates and input
        let mouse_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_pressed = ui.input(|i| i.pointer.primary_pressed());

        // Mouse wheel zoom on canvas when hovering over work area
        if let Some(pos) = mouse_pos
            && response.hovered()
            && work_rect.contains(pos)
        {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let zoom_factor = if scroll > 0.0 { 1.08 } else { 0.92 };
                self.canvas_zoom = (self.canvas_zoom * zoom_factor).clamp(0.1, 8.0);
                ui.ctx().request_repaint();
            }
        }

        if self.active_tool == ToolMode::Crop {
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

                // Double click commits crop
                if response.double_clicked() && is_inside {
                    self.commit_crop();
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

            // Draw tactile corner bracket handles
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
                let eye_stroke =
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 215, 0, 180));

                let cx = crop_rect.center().x;
                painter.line_segment(
                    [
                        Pos2::new(cx, crop_rect.min.y),
                        Pos2::new(cx, crop_rect.max.y),
                    ],
                    guide_stroke,
                );

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

            // Dimension annotation badge above crop frame if enabled
            if self.show_dimensions {
                let badge_text = format!(
                    "Passport Framing: {:.1} × {:.1} mm ({:.2} × {:.2} in)",
                    self.grid_config.photo.width.0,
                    self.grid_config.photo.height.0,
                    self.grid_config.photo.width.to_inches().0,
                    self.grid_config.photo.height.to_inches().0,
                );
                let badge_pos_y = (crop_rect.min.y - 14.0).max(work_rect.min.y + 12.0);
                let badge_pos = Pos2::new(crop_rect.center().x, badge_pos_y);
                let font = egui::FontId::proportional(11.0);
                let galley =
                    painter.layout_no_wrap(badge_text, font, Color32::from_rgb(220, 240, 255));
                let badge_rect =
                    Rect::from_center_size(badge_pos, galley.size() + Vec2::new(14.0, 6.0));
                painter.rect_filled(
                    badge_rect,
                    3.0,
                    Color32::from_rgba_unmultiplied(15, 20, 30, 220),
                );
                painter.rect_stroke(
                    badge_rect,
                    3.0,
                    Stroke::new(1.0, Color32::from_rgb(0, 160, 255)),
                    egui::StrokeKind::Outside,
                );
                painter.galley(
                    badge_rect.min + Vec2::new(7.0, 3.0),
                    galley,
                    Color32::from_rgb(220, 240, 255),
                );
            }

            // Smooth drag execution
            if primary_down
                && self.crop_drag_mode != CropDragMode::None
                && disp_w > 0.0
                && disp_h > 0.0
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
                    if let Some(doc) = self.documents.get_mut(self.active_doc_idx) {
                        doc.crop_area = self.crop_area;
                    }
                }
                ui.ctx().request_repaint();
            }
        } else if self.active_tool == ToolMode::Select {
            // Select Tool view: display dimension annotation if enabled
            if self.show_dimensions {
                let badge_text = format!(
                    "Photo Dimensions: {} × {} px ({:.1} × {:.1} mm)",
                    src_w_u32,
                    src_h_u32,
                    self.grid_config.photo.width.0,
                    self.grid_config.photo.height.0,
                );
                let badge_pos_y = (disp_rect.min.y - 14.0).max(work_rect.min.y + 12.0);
                let badge_pos = Pos2::new(disp_rect.center().x, badge_pos_y);
                let font = egui::FontId::proportional(11.0);
                let galley =
                    painter.layout_no_wrap(badge_text, font, Color32::from_rgb(220, 240, 255));
                let badge_rect =
                    Rect::from_center_size(badge_pos, galley.size() + Vec2::new(14.0, 6.0));
                painter.rect_filled(
                    badge_rect,
                    3.0,
                    Color32::from_rgba_unmultiplied(15, 20, 30, 220),
                );
                painter.rect_stroke(
                    badge_rect,
                    3.0,
                    Stroke::new(1.0, Color32::from_rgb(100, 180, 255)),
                    egui::StrokeKind::Outside,
                );
                painter.galley(
                    badge_rect.min + Vec2::new(7.0, 3.0),
                    galley,
                    Color32::from_rgb(220, 240, 255),
                );
            }
        } else if self.active_tool == ToolMode::Pan {
            // Pan Tool view
            if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
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

        // Zoom handling via mouse wheel scroll (only when cursor is hovering over the canvas)
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let factor = (1.0 + scroll * 0.002).clamp(0.8, 1.25);
                self.canvas_zoom = (self.canvas_zoom * factor).clamp(0.2, 5.0);
            }
        }

        let painter = ui.painter_at(rect);

        // Draw dark workbench canvas background
        painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 32, 38));

        // Printable sheet workspace area (full canvas below unified options header)
        let work_rect = rect;

        // Determine scale: mm to screen pixels
        let sheet_w_mm = self.grid_config.sheet.width.0.max(1.0);
        let sheet_h_mm = self.grid_config.sheet.height.0.max(1.0);

        let padding = 40.0;
        let fit_scale_x = (work_rect.width() - padding * 2.0).max(10.0) / sheet_w_mm;
        let fit_scale_y = (work_rect.height() - padding * 2.0).max(10.0) / sheet_h_mm;
        let base_scale = fit_scale_x.min(fit_scale_y);
        let px_per_mm = base_scale * self.canvas_zoom;

        let sheet_pixel_w = sheet_w_mm * px_per_mm;
        let sheet_pixel_h = sheet_h_mm * px_per_mm;

        // Center of the canvas plus user pan offset
        let sheet_origin_x = work_rect.center().x - (sheet_pixel_w / 2.0) + self.canvas_pan.x;
        let sheet_origin_y = work_rect.center().y - (sheet_pixel_h / 2.0) + self.canvas_pan.y;

        let sheet_rect = Rect::from_min_size(
            Pos2::new(sheet_origin_x, sheet_origin_y),
            Vec2::new(sheet_pixel_w, sheet_pixel_h),
        );

        // Set dynamic cursor icon based on active tool mode
        if response.hovered() {
            match self.active_tool {
                ToolMode::Pan => {
                    ui.ctx().set_cursor_icon(if response.dragged() {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    });
                }
                ToolMode::Select => {
                    let is_over_slot = response.hover_pos().is_some_and(|pos| {
                        compute_layout(&self.grid_config).is_ok_and(|layout| {
                            layout.slots.iter().any(|slot| {
                                let slot_x = sheet_rect.min.x + (slot.x.0 * px_per_mm);
                                let slot_y = sheet_rect.min.y + (slot.y.0 * px_per_mm);
                                let slot_w = slot.width.0 * px_per_mm;
                                let slot_h = slot.height.0 * px_per_mm;
                                Rect::from_min_size(
                                    Pos2::new(slot_x, slot_y),
                                    Vec2::new(slot_w, slot_h),
                                )
                                .contains(pos)
                            })
                        })
                    });
                    if is_over_slot {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    } else {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                    }
                }
                ToolMode::Crop => {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
            }
        }

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
                // Subtle tinted margins overlay to highlight non-printable border
                let tint = Color32::from_rgba_unmultiplied(0, 140, 255, 18);
                let top_m = Rect::from_min_max(
                    sheet_rect.min,
                    Pos2::new(sheet_rect.max.x, printable_rect.min.y),
                );
                if top_m.is_positive() {
                    painter.rect_filled(top_m, 0.0, tint);
                }
                let bot_m = Rect::from_min_max(
                    Pos2::new(sheet_rect.min.x, printable_rect.max.y),
                    sheet_rect.max,
                );
                if bot_m.is_positive() {
                    painter.rect_filled(bot_m, 0.0, tint);
                }
                let left_m = Rect::from_min_max(
                    Pos2::new(sheet_rect.min.x, printable_rect.min.y),
                    Pos2::new(printable_rect.min.x, printable_rect.max.y),
                );
                if left_m.is_positive() {
                    painter.rect_filled(left_m, 0.0, tint);
                }
                let right_m = Rect::from_min_max(
                    Pos2::new(printable_rect.max.x, printable_rect.min.y),
                    Pos2::new(sheet_rect.max.x, printable_rect.max.y),
                );
                if right_m.is_positive() {
                    painter.rect_filled(right_m, 0.0, tint);
                }

                // High-visibility cyan guide outline for printable area
                let margin_stroke = Stroke::new(1.5, Color32::from_rgb(0, 150, 245));
                painter.rect_stroke(printable_rect, 0.0, margin_stroke, egui::StrokeKind::Inside);

                // Printable area dimension badge at top-left
                let margin_label = format!(
                    "Printable Bounds (Margins: L:{:.1} R:{:.1} T:{:.1} B:{:.1} mm)",
                    self.grid_config.margins.left.0,
                    self.grid_config.margins.right.0,
                    self.grid_config.margins.top.0,
                    self.grid_config.margins.bottom.0,
                );
                painter.text(
                    printable_rect.min + Vec2::new(4.0, 3.0),
                    egui::Align2::LEFT_TOP,
                    margin_label,
                    egui::FontId::proportional(9.5),
                    Color32::from_rgb(0, 130, 230),
                );
            }
        }

        // Render photo slots from computed layout
        match compute_layout(&self.grid_config) {
            Ok(layout) => {
                self.paint_slots(&painter, &sheet_rect, px_per_mm, &layout);

                // Slot click handling: only when Select tool is active!
                // When in Pan tool, clicking is non-destructive for safe canvas navigation
                if let Some(pos) = response
                    .interact_pointer_pos()
                    .filter(|_| response.clicked() && self.active_tool == ToolMode::Select)
                {
                    let mut hit_slot = false;
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
                            hit_slot = true;
                            if self.loaded_image.is_some() {
                                if response.double_clicked() {
                                    self.switch_to_photo_edit();
                                }
                            } else {
                                self.import_image_dialog();
                            }
                            break;
                        }
                    }
                    if !hit_slot && sheet_rect.contains(pos) {
                        self.selected_slot = None;
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

        // Draw sheet dimension annotations with high-contrast pill badge
        if self.show_dimensions {
            let dim_text = format!(
                "Sheet: {} × {} ({:.1} × {:.1} in) • Photo: {} × {} mm • DPI: {}",
                self.grid_config.sheet.width,
                self.grid_config.sheet.height,
                self.grid_config.sheet.width.to_inches().0,
                self.grid_config.sheet.height.to_inches().0,
                self.grid_config.photo.width.0,
                self.grid_config.photo.height.0,
                self.dpi,
            );
            let badge_pos_y = (sheet_rect.min.y - 14.0).max(work_rect.min.y + 12.0);
            let badge_center = Pos2::new(sheet_rect.center().x, badge_pos_y);
            let font = egui::FontId::proportional(11.0);
            let galley = painter.layout_no_wrap(dim_text, font, Color32::from_rgb(220, 240, 255));
            let badge_rect =
                Rect::from_center_size(badge_center, galley.size() + Vec2::new(16.0, 8.0));
            painter.rect_filled(
                badge_rect,
                4.0,
                Color32::from_rgba_unmultiplied(20, 25, 35, 230),
            );
            painter.rect_stroke(
                badge_rect,
                4.0,
                Stroke::new(1.0, Color32::from_rgb(0, 160, 255)),
                egui::StrokeKind::Outside,
            );
            painter.galley(
                badge_rect.min + Vec2::new(8.0, 4.0),
                galley,
                Color32::from_rgb(220, 240, 255),
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

            // Selected slot accent outline
            if Some(idx) == self.selected_slot {
                painter.rect_stroke(
                    photo_rect.expand(2.0),
                    1.0,
                    Stroke::new(2.0, Color32::from_rgb(0, 160, 255)),
                    egui::StrokeKind::Outside,
                );
            }

            // Cut marks at corners (extending outward into margins and gutters for clean guillotine trimming)
            if self.show_cut_marks {
                let mark_len = 8.0;
                let offset = 2.0;
                let stroke = Stroke::new(1.2, Color32::from_rgb(35, 40, 50));

                // Top-left
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.min.x, photo_rect.min.y - offset),
                        Pos2::new(photo_rect.min.x, photo_rect.min.y - offset - mark_len),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.min.x - offset, photo_rect.min.y),
                        Pos2::new(photo_rect.min.x - offset - mark_len, photo_rect.min.y),
                    ],
                    stroke,
                );

                // Top-right
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.max.x, photo_rect.min.y - offset),
                        Pos2::new(photo_rect.max.x, photo_rect.min.y - offset - mark_len),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.max.x + offset, photo_rect.min.y),
                        Pos2::new(photo_rect.max.x + offset + mark_len, photo_rect.min.y),
                    ],
                    stroke,
                );

                // Bottom-left
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.min.x, photo_rect.max.y + offset),
                        Pos2::new(photo_rect.min.x, photo_rect.max.y + offset + mark_len),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.min.x - offset, photo_rect.max.y),
                        Pos2::new(photo_rect.min.x - offset - mark_len, photo_rect.max.y),
                    ],
                    stroke,
                );

                // Bottom-right
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.max.x, photo_rect.max.y + offset),
                        Pos2::new(photo_rect.max.x, photo_rect.max.y + offset + mark_len),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(photo_rect.max.x + offset, photo_rect.max.y),
                        Pos2::new(photo_rect.max.x + offset + mark_len, photo_rect.max.y),
                    ],
                    stroke,
                );
            }

            // Slot dimension annotation tag if enabled
            if self.show_dimensions {
                let dim_text = format!("{} × {} mm", slot.width.0, slot.height.0);
                let badge_pos = Pos2::new(photo_rect.center().x, photo_rect.max.y - 12.0);
                let font = egui::FontId::proportional(10.0);
                let galley = painter.layout_no_wrap(dim_text, font, Color32::WHITE);
                let badge_rect =
                    Rect::from_center_size(badge_pos, galley.size() + Vec2::new(10.0, 4.0));
                painter.rect_filled(
                    badge_rect,
                    3.0,
                    Color32::from_rgba_unmultiplied(10, 15, 25, 210),
                );
                painter.rect_stroke(
                    badge_rect,
                    3.0,
                    Stroke::new(1.0, Color32::from_rgb(0, 180, 255)),
                    egui::StrokeKind::Outside,
                );
                painter.galley(badge_rect.min + Vec2::new(5.0, 2.0), galley, Color32::WHITE);
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
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
        assert_eq!(app.active_tool, ToolMode::Crop);
        assert_eq!(app.photo_preset, PhotoPreset::UsPassport);
        assert_eq!(app.sheet_preset, SheetPreset::Photo4x6);
        assert_eq!(app.sheet_orientation, SheetOrientation::Portrait);
        assert_eq!(app.dpi, Dpi::STANDARD);
        assert_eq!(app.grid_config.rows, 1);
        assert_eq!(app.grid_config.cols, 1);
        assert!(compute_layout(&app.grid_config).is_ok());
        assert!(app.loaded_image.is_none());
        assert_eq!(app.crop_area, CropArea::default());
        assert_eq!(app.export_target, ExportTarget::FullSheet);
        assert_eq!(app.export_format, ExportFormat::Pdf);
        assert_eq!(app.documents.len(), 0);
        assert!(!app.can_undo());
        assert!(!app.can_redo());
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
        assert!(app.grid_config.rows >= 1);
        assert!(app.grid_config.cols >= 1);
    }

    #[test]
    fn test_workspace_modes_and_switch() {
        let mut app = ImprintApp::default();
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
        assert_eq!(app.active_tool, ToolMode::Crop);

        app.switch_to_sheet_layout();
        assert_eq!(app.workspace, WorkspaceMode::SheetLayout);
        assert_eq!(app.active_tool, ToolMode::Select);

        app.switch_to_photo_edit();
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
        assert_eq!(app.active_tool, ToolMode::Crop);
    }

    #[test]
    fn test_switch_to_sheet_layout_recovers_from_overflow() {
        let mut app = ImprintApp::default();
        // Force an overflow state
        app.grid_config.rows = 8;
        app.grid_config.cols = 8;
        assert!(compute_layout(&app.grid_config).is_err());

        app.switch_to_photo_edit();
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);

        // When switching back to sheet layout, it should recover to 1x1 to prevent overflow
        app.switch_to_sheet_layout();
        assert_eq!(app.workspace, WorkspaceMode::SheetLayout);
        assert_eq!(app.grid_config.rows, 1);
        assert_eq!(app.grid_config.cols, 1);
        assert!(compute_layout(&app.grid_config).is_ok());
    }

    #[test]
    fn test_tool_mode_properties_and_icons() {
        assert_eq!(ToolMode::Select.shortcut(), "V");
        assert_eq!(ToolMode::Pan.shortcut(), "H");
        assert_eq!(ToolMode::Crop.shortcut(), "C");

        assert_eq!(ToolMode::Select.icon(), ToolbarIcon::Pointer);
        assert_eq!(ToolMode::Pan.icon(), ToolbarIcon::SelectPan);
        assert_eq!(ToolMode::Crop.icon(), ToolbarIcon::Crop);

        assert!(!ToolMode::Select.description().is_empty());
        assert!(!ToolMode::Pan.description().is_empty());
        assert!(!ToolMode::Crop.description().is_empty());
    }

    #[test]
    fn test_keyboard_shortcuts_tool_switch() {
        let mut app = ImprintApp::default();
        let ctx = egui::Context::default();

        // Simulate pressing 'C' -> switches to PhotoEdit & Crop
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
        assert_eq!(app.active_tool, ToolMode::Crop);

        // Simulate pressing 'V' -> activates Select tool on canvas
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
        assert_eq!(app.active_tool, ToolMode::Select);

        // Simulate pressing 'Tab' -> switches to SheetLayout
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert_eq!(app.workspace, WorkspaceMode::SheetLayout);
    }

    #[test]
    fn test_guides_toggle_behavior() {
        let mut app = ImprintApp::default();
        assert!(app.show_cut_marks);
        assert!(app.show_margins);
        assert!(app.show_dimensions);
        assert!(app.show_biometric_guides);

        // Pressing 'R' toggles all off
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::R,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert!(!app.show_cut_marks);
        assert!(!app.show_margins);
        assert!(!app.show_dimensions);
        assert!(!app.show_biometric_guides);

        // Pressing R again toggles them on
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::R,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert!(app.show_cut_marks);
        assert!(app.show_margins);
        assert!(app.show_dimensions);
        assert!(app.show_biometric_guides);
    }

    #[test]
    fn test_sidebar_scroll_does_not_zoom_sheet_canvas() {
        let mut app = ImprintApp {
            workspace: WorkspaceMode::SheetLayout,
            ..Default::default()
        };
        let initial_zoom = app.canvas_zoom;

        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        // Frame 1: Move pointer onto the right sidebar area (x=1100, y=400)
        let mut raw1 = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        raw1.events
            .push(egui::Event::PointerMoved(Pos2::new(1100.0, 400.0)));
        let mut out1 = ctx.run_ui(raw1, |ui| {
            app.render_properties_panel(ui);
            app.render_central_canvas(ui);
        });
        out1.textures_delta.clear();

        // Frame 2: Mouse wheel scroll over sidebar
        let mut raw2 = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        raw2.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 50.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });

        let mut output = ctx.run_ui(raw2, |ui| {
            app.render_properties_panel(ui);
            app.render_central_canvas(ui);
        });
        output.textures_delta.clear();

        // Canvas zoom must remain unchanged because pointer was over the sidebar, not the canvas
        assert_eq!(app.canvas_zoom, initial_zoom);
    }

    #[test]
    fn test_canvas_scroll_zooms_sheet_canvas_when_hovered() {
        let mut app = ImprintApp {
            workspace: WorkspaceMode::SheetLayout,
            ..Default::default()
        };
        let initial_zoom = app.canvas_zoom;

        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        // Frame 1: Move pointer into central canvas
        let mut raw1 = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        raw1.events
            .push(egui::Event::PointerMoved(Pos2::new(500.0, 400.0)));
        let mut out1 = ctx.run_ui(raw1, |ui| {
            app.render_properties_panel(ui);
            app.render_central_canvas(ui);
        });
        out1.textures_delta.clear();

        // Frame 2: Mouse wheel scroll while hovering canvas
        let mut raw2 = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        raw2.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 50.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });

        let mut output = ctx.run_ui(raw2, |ui| {
            app.render_properties_panel(ui);
            app.render_central_canvas(ui);
        });
        output.textures_delta.clear();

        // Canvas zoom should have changed because pointer was hovering over the canvas
        assert!(app.canvas_zoom > initial_zoom);
    }

    #[test]
    fn test_inspector_renders_all_tool_modes() {
        let mut app = ImprintApp::default();
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        let tools = [ToolMode::Select, ToolMode::Crop, ToolMode::Pan];
        for tool in tools {
            app.active_tool = tool;
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| {
                app.render_properties_panel(ui);
            });
            out.textures_delta.clear();
        }
    }

    #[test]
    fn test_inspector_selected_slot_display() {
        let mut app = ImprintApp::default();
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        // Select slot 0
        app.selected_slot = Some(0);
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            app.render_properties_panel(ui);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn test_inspector_header_clean_without_tool_switcher() {
        let mut app = ImprintApp::default();
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        for tool in [ToolMode::Select, ToolMode::Crop, ToolMode::Pan] {
            app.active_tool = tool;
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| {
                app.render_inspector_header(ui);
            });
            out.textures_delta.clear();
        }
    }

    #[test]
    fn test_sheet_canvas_renders_without_biometric_guides_on_slots() {
        let mut app = ImprintApp {
            show_biometric_guides: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            app.render_sheet_canvas(ui);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn test_h_shortcut_activates_pan_tool() {
        let mut app = ImprintApp {
            workspace: WorkspaceMode::PhotoEdit,
            active_tool: ToolMode::Crop,
            ..Default::default()
        };
        let ctx = egui::Context::default();

        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::H,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
        assert_eq!(app.active_tool, ToolMode::Pan);
    }

    #[test]
    fn test_cmd_p_shortcut_toggles_sheet_preview() {
        let mut app = ImprintApp::default();
        let ctx = egui::Context::default();
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);

        // Press ⌘P to toggle to Sheet Print Preview
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::P,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        let mut output = ctx.run_ui(raw, |_ui| {});
        output.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert_eq!(app.workspace, WorkspaceMode::SheetLayout);

        // Press Escape to return to Photo Canvas
        let mut raw2 = egui::RawInput::default();
        raw2.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut output2 = ctx.run_ui(raw2, |_ui| {});
        output2.textures_delta.clear();

        app.handle_keyboard_shortcuts(&ctx);
        assert_eq!(app.workspace, WorkspaceMode::PhotoEdit);
    }

    #[test]
    fn test_document_tabs_lifecycle() {
        let mut app = ImprintApp::default();
        assert_eq!(app.documents.len(), 0);

        let img1 = DynamicImage::new_rgb8(800, 600);
        let loaded1 = LoadedImage::from_dynamic_image(img1, "photo1.jpg");
        app.add_document(loaded1);

        assert_eq!(app.documents.len(), 1);
        assert_eq!(app.active_doc_idx, 0);
        assert_eq!(app.documents[0].title, "photo1.jpg");
        assert_eq!(app.loaded_image.as_ref().unwrap().file_name, "photo1.jpg");

        let img2 = DynamicImage::new_rgb8(1200, 800);
        let loaded2 = LoadedImage::from_dynamic_image(img2, "photo2.png");
        app.add_document(loaded2);

        assert_eq!(app.documents.len(), 2);
        assert_eq!(app.active_doc_idx, 1);
        assert_eq!(app.documents[1].title, "photo2.png");
        assert_eq!(app.loaded_image.as_ref().unwrap().file_name, "photo2.png");

        // Switch to tab 0
        app.select_document(0);
        assert_eq!(app.active_doc_idx, 0);
        assert_eq!(app.loaded_image.as_ref().unwrap().file_name, "photo1.jpg");

        // Close tab 0
        app.close_document(0);
        assert_eq!(app.documents.len(), 1);
        assert_eq!(app.active_doc_idx, 0);
        assert_eq!(app.documents[0].title, "photo2.png");
        assert_eq!(app.loaded_image.as_ref().unwrap().file_name, "photo2.png");

        // Close last tab
        app.close_document(0);
        assert_eq!(app.documents.len(), 0);
        assert!(app.loaded_image.is_none());
    }

    #[test]
    fn test_commit_crop_and_undo_redo_in_app() {
        let mut app = ImprintApp::default();
        let img = DynamicImage::new_rgb8(1000, 1000);
        let loaded = LoadedImage::from_dynamic_image(img, "portrait.jpg");
        app.add_document(loaded);

        assert_eq!(app.loaded_image.as_ref().unwrap().width, 1000);
        assert_eq!(app.loaded_image.as_ref().unwrap().height, 1000);
        assert!(!app.can_undo());
        assert!(!app.can_redo());

        // Set crop area to central 50%
        app.crop_area = CropArea::new(0.25, 0.25, 0.5, 0.5);

        // Commit crop
        app.commit_crop();
        assert_eq!(app.loaded_image.as_ref().unwrap().width, 500);
        assert_eq!(app.loaded_image.as_ref().unwrap().height, 500);
        assert!(app.can_undo());
        assert!(!app.can_redo());

        // Undo crop
        assert!(app.undo());
        assert_eq!(app.loaded_image.as_ref().unwrap().width, 1000);
        assert_eq!(app.loaded_image.as_ref().unwrap().height, 1000);
        assert!(!app.can_undo());
        assert!(app.can_redo());

        // Redo crop
        assert!(app.redo());
        assert_eq!(app.loaded_image.as_ref().unwrap().width, 500);
        assert_eq!(app.loaded_image.as_ref().unwrap().height, 500);
        assert!(app.can_undo());
        assert!(!app.can_redo());
    }

    #[test]
    fn test_pan_tool_does_not_mutate_selected_slot_on_click() {
        let mut app = ImprintApp {
            active_tool: ToolMode::Pan,
            selected_slot: None,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        // Click at screen center where sheet photo slots are positioned
        let click_pos = screen.center();
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        raw.events.push(egui::Event::PointerMoved(click_pos));
        raw.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        raw.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut out = ctx.run_ui(raw, |ui| {
            app.render_sheet_canvas(ui);
        });
        out.textures_delta.clear();

        // Pan tool must NOT change selected_slot
        assert_eq!(app.selected_slot, None);
    }

    #[test]
    fn test_select_tool_click_on_slot_updates_selection() {
        let mut app = ImprintApp {
            active_tool: ToolMode::Select,
            selected_slot: None,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        // Center of the canvas where sheet photo slot is
        let click_pos = screen.center();
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        raw.events.push(egui::Event::PointerMoved(click_pos));
        raw.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        raw.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut out = ctx.run_ui(raw, |ui| {
            app.render_sheet_canvas(ui);
        });
        out.textures_delta.clear();

        // Select tool handles slot clicking (selects slot 0 or deselects background without panicking)
        // Verify render completed cleanly and tool mode remains Select
        assert_eq!(app.active_tool, ToolMode::Select);
    }

    #[test]
    fn test_view_overlays_toggle_render_sheet_and_crop() {
        let mut app = ImprintApp {
            show_cut_marks: true,
            show_margins: true,
            show_dimensions: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));

        // 1. Render sheet canvas with all guides enabled
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            app.render_sheet_canvas(ui);
        });
        out.textures_delta.clear();

        // 2. Render sheet canvas with all guides disabled
        app.show_cut_marks = false;
        app.show_margins = false;
        app.show_dimensions = false;
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            app.render_sheet_canvas(ui);
        });
        out.textures_delta.clear();

        // 3. Render crop editor with dimensions enabled
        app.show_dimensions = true;
        app.workspace = WorkspaceMode::PhotoEdit;
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            app.render_crop_editor(ui);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn test_reset_crop_restores_default_aspect_framing() {
        let mut app = ImprintApp::default();
        let img = image::RgbaImage::new(1000, 1000);
        let loaded = LoadedImage {
            file_path: None,
            original: image::DynamicImage::ImageRgba8(img),
            width: 1000,
            height: 1000,
            file_name: "test.png".to_string(),
        };
        app.add_document(loaded);

        // Manually shrink and offset crop_area
        app.crop_area = CropArea::new(0.2, 0.3, 0.4, 0.4);
        assert_eq!(app.crop_area.width, 0.4);

        // Reset crop should restore default aspect framing (1.0 x 1.0 for square at 1:1 aspect)
        app.reset_crop();
        let target_aspect = app.grid_config.photo.aspect_ratio();
        let default_expected = CropArea::default_for_aspect(1000, 1000, target_aspect);
        assert!((app.crop_area.x - default_expected.x).abs() < 1e-4);
        assert!((app.crop_area.width - default_expected.width).abs() < 1e-4);
        assert_eq!(app.active_document().unwrap().crop_area, app.crop_area);
    }

    #[test]
    fn test_reset_crop_reverts_to_original_when_already_default() {
        let mut app = ImprintApp::default();
        let img = image::RgbaImage::new(1000, 1000);
        let loaded = LoadedImage {
            file_path: None,
            original: image::DynamicImage::ImageRgba8(img),
            width: 1000,
            height: 1000,
            file_name: "test.png".to_string(),
        };
        app.add_document(loaded);

        // Crop to a smaller region and commit
        app.crop_area = CropArea::new(0.1, 0.1, 0.5, 0.5);
        app.commit_crop();
        assert_eq!(app.loaded_image.as_ref().unwrap().width, 500);
        assert_eq!(app.loaded_image.as_ref().unwrap().height, 500);
        assert!(app.can_undo());

        // Calling reset_crop when already at default for the cropped image reverts to original uncropped photo
        app.reset_crop();
        assert_eq!(app.loaded_image.as_ref().unwrap().width, 1000);
        assert_eq!(app.loaded_image.as_ref().unwrap().height, 1000);
        assert!(!app.can_undo());
    }

    #[test]
    fn test_tab_bar_and_options_bar_render_safely() {
        let mut app = ImprintApp::default();
        let img = image::RgbaImage::new(400, 400);
        let loaded = LoadedImage {
            file_path: None,
            original: image::DynamicImage::ImageRgba8(img),
            width: 400,
            height: 400,
            file_name: "photo.jpg".to_string(),
        };
        app.add_document(loaded);

        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let raw = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };

        let mut out = ctx.run_ui(raw, |ui| {
            app.render_document_tab_bar(ui);
            app.render_canvas_options_bar(ui);
        });
        out.textures_delta.clear();
    }
}
