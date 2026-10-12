//! Main GUI application state and layout rendering using `egui` and `eframe`.

use crate::layout::{
    GridConfig, GuttersMm, LayoutGeometry, MarginsMm, calculate_max_fit, compute_layout,
};
use crate::preset::{PhotoPreset, SheetOrientation, SheetPreset};
use crate::units::{DimensionsMm, Dpi, Millimeters};
use eframe::egui::{self, Color32, Panel, Pos2, Rect, Stroke, Ui, Vec2};

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

/// The main Imprint application state.
pub struct ImprintApp {
    // Presets & orientation
    pub photo_preset: PhotoPreset,
    pub sheet_preset: SheetPreset,
    pub sheet_orientation: SheetOrientation,
    pub dpi: Dpi,

    // Layout configuration
    pub grid_config: GridConfig,

    // Interaction & View
    pub active_tool: ToolMode,
    pub canvas_zoom: f32,
    pub canvas_pan: Vec2,

    // View Options
    pub show_margins: bool,
    pub show_cut_marks: bool,
    pub show_dimensions: bool,

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
            active_tool: ToolMode::SelectPan,
            canvas_zoom: 1.0,
            canvas_pan: Vec2::ZERO,
            show_margins: true,
            show_cut_marks: true,
            show_dimensions: true,
            status_message: "Ready. Configure layout or use mouse to pan and zoom canvas."
                .to_string(),
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
}

impl eframe::App for ImprintApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
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
                        self.status_message = "Image import dialog requested.".to_string();
                        ui.close();
                    }
                    if ui.button("Export Print Layout...").clicked() {
                        self.status_message = "Print export requested.".to_string();
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
            .default_size(300.0)
            .min_size(260.0)
            .max_size(450.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.heading("Properties & Settings");
                    ui.add_space(8.0);

                    // --- Sheet Setup ---
                    ui.group(|ui| {
                        ui.strong("📄 Sheet Setup");
                        ui.add_space(4.0);

                        let old_preset = self.sheet_preset;
                        egui::ComboBox::from_label("Preset")
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

                        // Orientation toggle
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
                        egui::ComboBox::from_label("Standard")
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
                            }
                        });

                        let aspect = self.grid_config.photo.aspect_ratio();
                        ui.label(format!("Aspect Ratio: {:.2} : 1", aspect));

                        ui.horizontal(|ui| {
                            ui.label("Print DPI:");
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
                        });

                        let (px_w, px_h) = self.grid_config.photo.to_pixels(self.dpi);
                        ui.label(format!("Output Size: {} × {}", px_w, px_h));
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
                            if ui.button("⚡ Calculate Max Fit").clicked() {
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

                    // --- Layout Verification & Status Summary ---
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

    /// Renders the central canvas with interactive sheet preview.
    fn render_central_canvas(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            let available_size = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available_size, egui::Sense::drag());

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
            // At zoom 1.0, map sheet to fit comfortably in central panel with margins
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
        });
    }

    /// Paints photo slots and crop/cut guides onto the canvas.
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

            // Photo card background
            painter.rect_filled(photo_rect, 0.0, Color32::from_rgb(245, 247, 250));
            // Photo card boundary border
            painter.rect_stroke(
                photo_rect,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(100, 130, 180)),
                egui::StrokeKind::Inside,
            );

            // Photo center placeholder watermark / index
            let label = format!("#{}\n{} × {}", idx + 1, slot.width, slot.height);
            painter.text(
                photo_rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(120, 140, 170),
            );

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
