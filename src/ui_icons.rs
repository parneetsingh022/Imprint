//! Vector-drawn professional toolbar icons and Photoshop-style tool buttons.
//!
//! Replaces hardcoded emoji / UTF-8 glyphs with crisp, resolution-independent
//! geometric vector drawings rendered via `egui::Painter`.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Stroke, StrokeKind, Ui, Vec2};

/// Available icons for the Photoshop-style vertical toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarIcon {
    /// Pointer / selection arrow cursor (Photoshop Selection V).
    Pointer,
    /// Pan / Selection / Move tool (Photoshop Move 4-way crosshair).
    SelectPan,
    /// Interactive Crop tool (Photoshop interlocking L-angle crop frame).
    Crop,
    /// Multi-photo print grid layout (2x2 photo tiles).
    Grid,
    /// Precision ruler & biometric guide marks.
    Guides,
    /// Import photograph (Photo frame with landscape peak & sun).
    Import,
    /// Export print-ready file (Tray with upward arrow).
    Export,
    /// Magnifier with plus.
    ZoomIn,
    /// Magnifier with minus.
    ZoomOut,
    /// 100% actual size / 1:1 pixel badge.
    Zoom100,
    /// Fit sheet canvas to viewport (Frame with 4 outward expansion corners).
    FitScreen,
    /// Undo previous edit (counter-clockwise arched arrow pointing left).
    Undo,
    /// Redo reverted edit (clockwise arched arrow pointing right).
    Redo,
}

/// Renders a crisp vector icon into the given painter centered at `center`.
pub fn draw_toolbar_icon(
    painter: &egui::Painter,
    icon: ToolbarIcon,
    center: Pos2,
    color: Color32,
    size: f32,
) {
    let s = size / 20.0;

    match icon {
        ToolbarIcon::Pointer => {
            // Sleek selection pointer arrow (Photoshop / Illustrator Selection V)
            let stroke = Stroke::new(1.4 * s, color);
            let p0 = center + Vec2::new(-5.5 * s, -7.0 * s); // Tip
            let p1 = center + Vec2::new(4.5 * s, 2.0 * s); // Bottom-right wing
            let p2 = center + Vec2::new(-0.5 * s, 2.0 * s); // Notch
            let p3 = center + Vec2::new(2.5 * s, 7.5 * s); // Stem tip
            let p4 = center + Vec2::new(0.5 * s, 8.5 * s); // Stem base
            let p5 = center + Vec2::new(-2.0 * s, 3.2 * s); // Stem return
            let p6 = center + Vec2::new(-5.5 * s, 4.5 * s); // Left wing

            let points = [p0, p1, p2, p3, p4, p5, p6];
            painter.add(egui::Shape::convex_polygon(
                points.to_vec(),
                color.gamma_multiply(0.25),
                stroke,
            ));
        }

        ToolbarIcon::SelectPan => {
            // Photoshop Move tool: 4-way direction crosshair with arrowheads
            let stem_stroke = Stroke::new(1.6 * s, color);
            let arrow_stroke = Stroke::new(1.4 * s, color);

            // Horizontal & vertical stems
            painter.line_segment(
                [
                    center - Vec2::new(6.5 * s, 0.0),
                    center + Vec2::new(6.5 * s, 0.0),
                ],
                stem_stroke,
            );
            painter.line_segment(
                [
                    center - Vec2::new(0.0, 6.5 * s),
                    center + Vec2::new(0.0, 6.5 * s),
                ],
                stem_stroke,
            );

            // Top arrow
            painter.line_segment(
                [
                    center + Vec2::new(-2.5 * s, -4.5 * s),
                    center + Vec2::new(0.0, -7.5 * s),
                ],
                arrow_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(2.5 * s, -4.5 * s),
                    center + Vec2::new(0.0, -7.5 * s),
                ],
                arrow_stroke,
            );

            // Bottom arrow
            painter.line_segment(
                [
                    center + Vec2::new(-2.5 * s, 4.5 * s),
                    center + Vec2::new(0.0, 7.5 * s),
                ],
                arrow_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(2.5 * s, 4.5 * s),
                    center + Vec2::new(0.0, 7.5 * s),
                ],
                arrow_stroke,
            );

            // Left arrow
            painter.line_segment(
                [
                    center + Vec2::new(-4.5 * s, -2.5 * s),
                    center + Vec2::new(-7.5 * s, 0.0),
                ],
                arrow_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-4.5 * s, 2.5 * s),
                    center + Vec2::new(-7.5 * s, 0.0),
                ],
                arrow_stroke,
            );

            // Right arrow
            painter.line_segment(
                [
                    center + Vec2::new(4.5 * s, -2.5 * s),
                    center + Vec2::new(7.5 * s, 0.0),
                ],
                arrow_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(4.5 * s, 2.5 * s),
                    center + Vec2::new(7.5 * s, 0.0),
                ],
                arrow_stroke,
            );

            // Center pivot dot
            painter.circle_filled(center, 1.4 * s, color);
        }

        ToolbarIcon::Crop => {
            // Photoshop Crop Tool: Two interlocking L-angles with overlapping extended arms
            let stroke = Stroke::new(1.8 * s, color);

            // Top-left L-bracket with extended tails
            painter.line_segment(
                [
                    center + Vec2::new(-7.5 * s, -4.5 * s),
                    center + Vec2::new(4.5 * s, -4.5 * s),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-4.5 * s, -7.5 * s),
                    center + Vec2::new(-4.5 * s, 4.5 * s),
                ],
                stroke,
            );

            // Bottom-right L-bracket with extended tails
            painter.line_segment(
                [
                    center + Vec2::new(-4.5 * s, 4.5 * s),
                    center + Vec2::new(7.5 * s, 4.5 * s),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(4.5 * s, -4.5 * s),
                    center + Vec2::new(4.5 * s, 7.5 * s),
                ],
                stroke,
            );

            // Subtle diagonal guide mark
            let guide_stroke = Stroke::new(1.0 * s, color.gamma_multiply(0.4));
            painter.line_segment(
                [
                    center + Vec2::new(-3.0 * s, -3.0 * s),
                    center + Vec2::new(3.0 * s, 3.0 * s),
                ],
                guide_stroke,
            );
        }

        ToolbarIcon::Grid => {
            // 2x2 photo tiles layout representing printable photo grid
            let tile_stroke = Stroke::new(1.4 * s, color);
            let fill = color.gamma_multiply(0.2);

            let tiles = [
                Rect::from_min_max(
                    center + Vec2::new(-7.0 * s, -7.0 * s),
                    center + Vec2::new(-1.2 * s, -1.2 * s),
                ),
                Rect::from_min_max(
                    center + Vec2::new(1.2 * s, -7.0 * s),
                    center + Vec2::new(7.0 * s, -1.2 * s),
                ),
                Rect::from_min_max(
                    center + Vec2::new(-7.0 * s, 1.2 * s),
                    center + Vec2::new(-1.2 * s, 7.0 * s),
                ),
                Rect::from_min_max(
                    center + Vec2::new(1.2 * s, 1.2 * s),
                    center + Vec2::new(7.0 * s, 7.0 * s),
                ),
            ];

            for r in tiles {
                painter.rect_filled(r, 1.0, fill);
                painter.rect_stroke(r, 1.0, tile_stroke, StrokeKind::Inside);
            }
        }

        ToolbarIcon::Guides => {
            // Precision ruler with measurement ticks
            let body = Rect::from_min_max(
                center + Vec2::new(-7.5 * s, -4.5 * s),
                center + Vec2::new(7.5 * s, 4.5 * s),
            );
            painter.rect_stroke(body, 1.0, Stroke::new(1.4 * s, color), StrokeKind::Inside);
            painter.rect_filled(body, 1.0, color.gamma_multiply(0.15));

            let tick_stroke = Stroke::new(1.2 * s, color);

            // Major ticks (top edge)
            for &x_off in &[-5.0, 0.0, 5.0] {
                painter.line_segment(
                    [
                        center + Vec2::new(x_off * s, -4.5 * s),
                        center + Vec2::new(x_off * s, -0.5 * s),
                    ],
                    tick_stroke,
                );
            }

            // Minor ticks
            for &x_off in &[-2.5, 2.5] {
                painter.line_segment(
                    [
                        center + Vec2::new(x_off * s, -4.5 * s),
                        center + Vec2::new(x_off * s, -2.5 * s),
                    ],
                    tick_stroke,
                );
            }
        }

        ToolbarIcon::Import => {
            // Photo card with mountain landscape and sun
            let frame = Rect::from_min_max(
                center + Vec2::new(-7.0 * s, -6.0 * s),
                center + Vec2::new(7.0 * s, 6.0 * s),
            );
            painter.rect_stroke(frame, 1.5, Stroke::new(1.4 * s, color), StrokeKind::Inside);
            painter.rect_filled(frame, 1.5, color.gamma_multiply(0.12));

            // Sun disk
            painter.circle_filled(center + Vec2::new(3.5 * s, -2.5 * s), 1.5 * s, color);

            // Mountain landscape lines
            let mtn_stroke = Stroke::new(1.4 * s, color);
            painter.line_segment(
                [
                    center + Vec2::new(-5.5 * s, 4.0 * s),
                    center + Vec2::new(-2.0 * s, 0.0 * s),
                ],
                mtn_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-2.0 * s, 0.0 * s),
                    center + Vec2::new(1.5 * s, 4.0 * s),
                ],
                mtn_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(0.5 * s, 4.0 * s),
                    center + Vec2::new(3.5 * s, 1.5 * s),
                ],
                mtn_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(3.5 * s, 1.5 * s),
                    center + Vec2::new(5.5 * s, 4.0 * s),
                ],
                mtn_stroke,
            );
        }

        ToolbarIcon::Export => {
            // Tray with upward export arrow
            let stroke = Stroke::new(1.5 * s, color);

            // Output base tray
            painter.line_segment(
                [
                    center + Vec2::new(-6.5 * s, 1.0 * s),
                    center + Vec2::new(-6.5 * s, 5.5 * s),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-6.5 * s, 5.5 * s),
                    center + Vec2::new(6.5 * s, 5.5 * s),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(6.5 * s, 5.5 * s),
                    center + Vec2::new(6.5 * s, 1.0 * s),
                ],
                stroke,
            );

            // Upward arrow shaft
            painter.line_segment(
                [
                    center + Vec2::new(0.0, 3.5 * s),
                    center + Vec2::new(0.0, -6.5 * s),
                ],
                stroke,
            );

            // Upward arrow head
            painter.line_segment(
                [
                    center + Vec2::new(-3.5 * s, -3.0 * s),
                    center + Vec2::new(0.0, -6.5 * s),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(3.5 * s, -3.0 * s),
                    center + Vec2::new(0.0, -6.5 * s),
                ],
                stroke,
            );
        }

        ToolbarIcon::ZoomIn => {
            // Magnifying glass with plus
            let stroke = Stroke::new(1.5 * s, color);
            let lens_center = center + Vec2::new(-1.8 * s, -1.8 * s);
            painter.circle_stroke(lens_center, 4.8 * s, stroke);

            // Handle
            painter.line_segment(
                [
                    center + Vec2::new(1.8 * s, 1.8 * s),
                    center + Vec2::new(6.8 * s, 6.8 * s),
                ],
                Stroke::new(2.2 * s, color),
            );

            // Plus mark inside lens
            painter.line_segment(
                [
                    lens_center - Vec2::new(2.2 * s, 0.0),
                    lens_center + Vec2::new(2.2 * s, 0.0),
                ],
                Stroke::new(1.3 * s, color),
            );
            painter.line_segment(
                [
                    lens_center - Vec2::new(0.0, 2.2 * s),
                    lens_center + Vec2::new(0.0, 2.2 * s),
                ],
                Stroke::new(1.3 * s, color),
            );
        }

        ToolbarIcon::ZoomOut => {
            // Magnifying glass with minus
            let stroke = Stroke::new(1.5 * s, color);
            let lens_center = center + Vec2::new(-1.8 * s, -1.8 * s);
            painter.circle_stroke(lens_center, 4.8 * s, stroke);

            // Handle
            painter.line_segment(
                [
                    center + Vec2::new(1.8 * s, 1.8 * s),
                    center + Vec2::new(6.8 * s, 6.8 * s),
                ],
                Stroke::new(2.2 * s, color),
            );

            // Minus mark inside lens
            painter.line_segment(
                [
                    lens_center - Vec2::new(2.2 * s, 0.0),
                    lens_center + Vec2::new(2.2 * s, 0.0),
                ],
                Stroke::new(1.3 * s, color),
            );
        }

        ToolbarIcon::Zoom100 => {
            // 1:1 Actual Size Badge
            let frame = Rect::from_min_max(
                center + Vec2::new(-7.5 * s, -5.5 * s),
                center + Vec2::new(7.5 * s, 5.5 * s),
            );
            painter.rect_filled(frame, 2.0, color.gamma_multiply(0.15));
            painter.rect_stroke(frame, 2.0, Stroke::new(1.2 * s, color), StrokeKind::Inside);

            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "1:1",
                FontId::proportional(9.0 * s),
                color,
            );
        }

        ToolbarIcon::FitScreen => {
            // Center page rectangle
            let inner = Rect::from_min_max(
                center + Vec2::new(-3.5 * s, -4.5 * s),
                center + Vec2::new(3.5 * s, 4.5 * s),
            );
            painter.rect_stroke(inner, 1.0, Stroke::new(1.2 * s, color), StrokeKind::Inside);
            painter.rect_filled(inner, 1.0, color.gamma_multiply(0.15));

            // 4 corner expansion brackets
            let bracket_stroke = Stroke::new(1.4 * s, color);

            // Top-left corner
            painter.line_segment(
                [
                    center + Vec2::new(-7.0 * s, -4.0 * s),
                    center + Vec2::new(-7.0 * s, -7.0 * s),
                ],
                bracket_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-7.0 * s, -7.0 * s),
                    center + Vec2::new(-4.0 * s, -7.0 * s),
                ],
                bracket_stroke,
            );

            // Top-right corner
            painter.line_segment(
                [
                    center + Vec2::new(4.0 * s, -7.0 * s),
                    center + Vec2::new(7.0 * s, -7.0 * s),
                ],
                bracket_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(7.0 * s, -7.0 * s),
                    center + Vec2::new(7.0 * s, -4.0 * s),
                ],
                bracket_stroke,
            );

            // Bottom-left corner
            painter.line_segment(
                [
                    center + Vec2::new(-7.0 * s, 4.0 * s),
                    center + Vec2::new(-7.0 * s, 7.0 * s),
                ],
                bracket_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-7.0 * s, 7.0 * s),
                    center + Vec2::new(-4.0 * s, 7.0 * s),
                ],
                bracket_stroke,
            );

            // Bottom-right corner
            painter.line_segment(
                [
                    center + Vec2::new(4.0 * s, 7.0 * s),
                    center + Vec2::new(7.0 * s, 7.0 * s),
                ],
                bracket_stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(7.0 * s, 7.0 * s),
                    center + Vec2::new(7.0 * s, 4.0 * s),
                ],
                bracket_stroke,
            );
        }

        ToolbarIcon::Undo => {
            // Arched counter-clockwise arrow pointing left
            let stroke = Stroke::new(1.6 * s, color);
            let tip = center + Vec2::new(-6.0 * s, -2.5 * s);
            painter.line_segment([tip, tip + Vec2::new(3.5 * s, -3.5 * s)], stroke);
            painter.line_segment([tip, tip + Vec2::new(3.5 * s, 3.5 * s)], stroke);

            let p0 = tip + Vec2::new(1.0 * s, 0.0);
            let p1 = center + Vec2::new(2.0 * s, -3.5 * s);
            let p2 = center + Vec2::new(6.0 * s, 1.0 * s);
            let p3 = center + Vec2::new(4.0 * s, 6.0 * s);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p3], stroke);
        }

        ToolbarIcon::Redo => {
            // Arched clockwise arrow pointing right
            let stroke = Stroke::new(1.6 * s, color);
            let tip = center + Vec2::new(6.0 * s, -2.5 * s);
            painter.line_segment([tip, tip + Vec2::new(-3.5 * s, -3.5 * s)], stroke);
            painter.line_segment([tip, tip + Vec2::new(-3.5 * s, 3.5 * s)], stroke);

            let p0 = tip - Vec2::new(1.0 * s, 0.0);
            let p1 = center + Vec2::new(-2.0 * s, -3.5 * s);
            let p2 = center + Vec2::new(-6.0 * s, 1.0 * s);
            let p3 = center + Vec2::new(-4.0 * s, 6.0 * s);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p3], stroke);
        }
    }
}

/// Renders a Photoshop-style icon-only toolbar button with rich hover tooltips.
pub fn render_toolbar_button(
    ui: &mut Ui,
    icon: ToolbarIcon,
    selected: bool,
    tooltip_title: &str,
    shortcut: Option<&str>,
    tooltip_desc: &str,
) -> Response {
    let btn_size = Vec2::splat(36.0);
    let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());

    let painter = ui.painter();

    // Determine colors based on interactive state
    let (bg_color, stroke, icon_color, show_indicator) = if selected {
        (
            Color32::from_rgb(38, 50, 72), // Deep professional studio slate
            Stroke::new(1.0, Color32::from_rgb(65, 115, 190)),
            Color32::WHITE,
            true,
        )
    } else if response.hovered() {
        (
            Color32::from_rgb(44, 46, 54), // Subtle highlight
            Stroke::new(1.0, Color32::from_rgb(60, 64, 76)),
            Color32::from_rgb(245, 248, 255),
            false,
        )
    } else {
        (
            Color32::TRANSPARENT,
            Stroke::NONE,
            Color32::from_rgb(160, 165, 178), // Neutral studio grey
            false,
        )
    };

    // Background tile
    painter.rect(rect, 4.0, bg_color, stroke, StrokeKind::Inside);

    // Selected indicator pill on left edge (Photoshop/Figma style)
    if show_indicator {
        let pill_x = rect.min.x + 1.5;
        let pill_stroke = Stroke::new(2.5, Color32::from_rgb(85, 155, 250));
        painter.line_segment(
            [
                Pos2::new(pill_x, rect.min.y + 6.0),
                Pos2::new(pill_x, rect.max.y - 6.0),
            ],
            pill_stroke,
        );
    }

    // Render vector icon centered in button
    draw_toolbar_icon(painter, icon, rect.center(), icon_color, 20.0);

    // Rich Photoshop-style tooltip on hover
    response.on_hover_ui(|ui| {
        ui.horizontal(|ui| {
            ui.strong(tooltip_title);
            if let Some(key) = shortcut {
                ui.colored_label(Color32::from_rgb(125, 175, 245), format!("[{}]", key));
            }
        });
        if !tooltip_desc.is_empty() {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(tooltip_desc)
                    .size(11.0)
                    .color(Color32::from_rgb(180, 185, 195)),
            );
        }
    })
}

/// Renders an options bar button with a vector icon, text label, and hover styling.
pub fn render_options_bar_button(
    ui: &mut Ui,
    icon: Option<ToolbarIcon>,
    text: &str,
    enabled: bool,
    tooltip: &str,
) -> Response {
    let height = 22.0;
    let font_id = egui::FontId::proportional(11.5);
    let text_color = if enabled {
        Color32::from_rgb(220, 225, 235)
    } else {
        Color32::from_rgb(105, 110, 120)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_id, text_color);

    let pad = 6.0;
    let icon_w = if icon.is_some() { 16.0 } else { 0.0 };
    let spacing = if icon.is_some() && !text.is_empty() {
        3.0
    } else {
        0.0
    };
    let total_w = pad * 2.0 + icon_w + spacing + galley.size().x;

    let (rect, response) = ui.allocate_exact_size(Vec2::new(total_w, height), egui::Sense::click());

    let painter = ui.painter();
    if enabled {
        let (bg, stroke) = if response.is_pointer_button_down_on() {
            (
                Color32::from_rgb(32, 34, 42),
                Stroke::new(1.0, Color32::from_rgb(80, 85, 95)),
            )
        } else if response.hovered() {
            (
                Color32::from_rgb(46, 50, 60),
                Stroke::new(1.0, Color32::from_rgb(72, 78, 90)),
            )
        } else {
            (
                Color32::from_rgb(36, 38, 46),
                Stroke::new(1.0, Color32::from_rgb(52, 56, 66)),
            )
        };
        painter.rect(rect, 3.0, bg, stroke, StrokeKind::Inside);

        let icon_color = if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(205, 210, 220)
        };
        let mut x_cursor = rect.min.x + pad;
        if let Some(ic) = icon {
            let icon_center = Pos2::new(x_cursor + icon_w / 2.0, rect.center().y);
            draw_toolbar_icon(painter, ic, icon_center, icon_color, 13.0);
            x_cursor += icon_w + spacing;
        }

        let text_color = if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(220, 225, 235)
        };
        let text_pos = Pos2::new(x_cursor, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, text_color);
    } else {
        painter.rect(
            rect,
            3.0,
            Color32::from_rgb(26, 28, 34),
            Stroke::new(1.0, Color32::from_rgb(38, 40, 48)),
            StrokeKind::Inside,
        );
        let mut x_cursor = rect.min.x + pad;
        if let Some(ic) = icon {
            let icon_center = Pos2::new(x_cursor + icon_w / 2.0, rect.center().y);
            draw_toolbar_icon(
                painter,
                ic,
                icon_center,
                Color32::from_rgb(75, 80, 90),
                13.0,
            );
            x_cursor += icon_w + spacing;
        }
        let text_pos = Pos2::new(x_cursor, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, Color32::from_rgb(85, 90, 100));
    }

    response.on_hover_text(tooltip).on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::NotAllowed
    })
}

/// Renders a horizontal divider line between toolbar sections.
pub fn render_toolbar_divider(ui: &mut Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, Color32::from_rgb(45, 48, 56)),
    );
    ui.add_space(3.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolbar_icons_draw_without_panicking() {
        // Construct a dummy Painter via egui context to verify all icon routines execute safely
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter();
            let icons = [
                ToolbarIcon::Pointer,
                ToolbarIcon::SelectPan,
                ToolbarIcon::Crop,
                ToolbarIcon::Grid,
                ToolbarIcon::Guides,
                ToolbarIcon::Import,
                ToolbarIcon::Export,
                ToolbarIcon::ZoomIn,
                ToolbarIcon::ZoomOut,
                ToolbarIcon::Zoom100,
                ToolbarIcon::FitScreen,
            ];

            for icon in icons {
                draw_toolbar_icon(painter, icon, Pos2::new(50.0, 50.0), Color32::WHITE, 20.0);
            }
        });
        output.textures_delta.clear();
    }
}
