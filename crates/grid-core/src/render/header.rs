use crate::canvas::CanvasCtx;
use crate::color::blend;
use crate::theme::Theme;
use crate::types::{ContentAlign, GridSelection, Rectangle};
use crate::walk::{walk_columns, walk_groups, MappedColumn};

use super::lib_utils::{get_middle_center_bias, rounded_rect};

/// Draw column headers and optionally group headers.
pub fn draw_grid_headers(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    enable_groups: bool,
    width: f64,
    translate_x: f64,
    header_height: f64,
    group_header_height: f64,
    selection: &GridSelection,
    theme: &Theme,
    vertical_border: impl Fn(usize) -> bool,
    get_group_details: impl Fn(&str) -> GroupDetails,
) {
    let total_header_height = header_height + group_header_height;
    if total_header_height <= 0.0 {
        return;
    }

    // Fill header background
    ctx.set_fill_style(&theme.bg_header);
    ctx.fill_rect(0.0, 0.0, width, total_header_height);

    let font = theme.header_font_full();
    let base_font = theme.base_font_full();
    ctx.set_font(&font);

    let bias = get_middle_center_bias(ctx, &base_font);

    // Draw group headers if enabled
    if enable_groups && group_header_height > 0.0 {
        draw_group_headers(ctx, effective_cols, width, translate_x, group_header_height, theme, &get_group_details);
    }

    // Draw column headers
    walk_columns(
        effective_cols,
        0,
        translate_x,
        0.0,
        total_header_height,
        |c, draw_x, _draw_y, clip_x, _start_row| {
            let diff = if clip_x > draw_x { clip_x - draw_x } else { 0.0 };
            let x = draw_x + diff;
            let w = c.width - diff;
            let y = if enable_groups { group_header_height } else { 0.0 };
            let h = header_height - y;

            if x > width || w <= 0.0 {
                return false;
            }

            // Clip to column bounds
            ctx.save();
            ctx.clip_rect(x, y, w, h);

            let is_selected = selection.columns.contains(&(c.source_index as i32));
            let has_selected_cell = selection.current.as_ref().map_or(false, |s| s.cell.col == c.source_index as i32);

            // Background
            if is_selected {
                ctx.set_fill_style(&theme.accent_color);
                ctx.fill_rect(x, y, w, h);
            } else if has_selected_cell {
                ctx.set_fill_style(&theme.bg_header_has_focus);
                ctx.fill_rect(x, y, w, h);
            }

            // Title text
            let text_color = if is_selected {
                &theme.text_header_selected
            } else {
                &theme.text_header
            };

            ctx.set_fill_style(text_color);
            ctx.set_font(&font);
            ctx.set_text_align("left");

            let text_x = x + theme.cell_horizontal_padding;
            let text_y = y + h / 2.0 + bias;
            let _ = ctx.fill_text(&c.title, text_x, text_y);

            // Right-aligned menu indicator (if applicable)
            // For now, skip menu icon rendering

            ctx.restore();

            // Draw separator line at left edge (except first col)
            if c.source_index > 0 {
                ctx.set_stroke_style(&theme.border_color);
                ctx.set_line_width(1.0);
                ctx.begin_path();
                ctx.move_to(x + 0.5, y);
                ctx.line_to(x + 0.5, y + h);
                ctx.stroke();
            }

            false
        },
    );
}

fn draw_group_headers(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    width: f64,
    translate_x: f64,
    group_header_height: f64,
    theme: &Theme,
    get_group_details: &impl Fn(&str) -> GroupDetails,
) {
    walk_groups(effective_cols, width, translate_x, group_header_height, |group_result| {
        if group_result.group.is_empty() {
            return;
        }

        let details = get_group_details(&group_result.group);
        let bg = details.override_theme.as_ref()
            .and_then(|t| t.bg_cell.as_deref())
            .unwrap_or_else(|| theme.bg_group_header());

        ctx.set_fill_style(bg);
        ctx.fill_rect(group_result.x, group_result.y, group_result.width, group_result.height);

        // Group title
        let text_color = details.override_theme.as_ref()
            .and_then(|t| t.text_dark.as_deref())
            .unwrap_or_else(|| theme.text_group_header());

        ctx.set_fill_style(text_color);
        let font = theme.base_font_full();
        ctx.set_font(&font);
        ctx.set_text_align("left");

        let text_x = group_result.x + theme.cell_horizontal_padding;
        let text_y = group_result.y + group_header_height / 2.0;
        let _ = ctx.fill_text(&group_result.group, text_x, text_y);

        // Border
        ctx.set_stroke_style(&theme.border_color);
        ctx.set_line_width(1.0);
        ctx.stroke_rect(group_result.x, group_result.y, group_result.width, group_result.height);
    });
}

pub struct GroupDetails {
    pub name: String,
    pub icon: Option<String>,
    pub override_theme: Option<crate::theme::ThemeOverride>,
}

impl Default for GroupDetails {
    fn default() -> Self {
        Self {
            name: String::new(),
            icon: None,
            override_theme: None,
        }
    }
}
