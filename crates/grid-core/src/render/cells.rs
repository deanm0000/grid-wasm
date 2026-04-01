use crate::canvas::CanvasCtx;
use crate::color::blend;
use crate::theme::Theme;
use crate::types::{ContentAlign, GridCell, GridSelection, Rectangle};
use crate::walk::{walk_columns, walk_rows_in_col, MappedColumn};

use super::lib_utils::{cell_is_selected, draw_text_cell, get_middle_center_bias, rounded_rect};

/// Draw all visible data cells.
pub fn draw_cells(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    height: f64,
    total_header_height: f64,
    translate_x: f64,
    translate_y: f64,
    cell_y_offset: usize,
    rows: usize,
    row_height: f64,
    get_cell_content: &dyn Fn(i32, i32) -> GridCell,
    selection: &GridSelection,
    freeze_trailing_rows: usize,
    has_append_row: bool,
    theme: &Theme,
    is_focused: bool,
    draw_focus: bool,
) {
    let font = theme.base_font_full();

    walk_columns(
        effective_cols,
        cell_y_offset,
        translate_x,
        translate_y,
        total_header_height,
        |c, draw_x, col_draw_y, clip_x, start_row| {
            let diff = if clip_x > draw_x { clip_x - draw_x } else { 0.0 };
            let col_draw_x = draw_x + diff;
            let col_draw_y = total_header_height + 1.0;
            let col_width = c.width - diff;
            let col_height = height - total_header_height - 1.0;

            if col_draw_x > height || col_width <= 0.0 {
                return false;
            }

            // Clip to column bounds
            ctx.save();
            ctx.clip_rect(col_draw_x, col_draw_y, col_width, col_height);

            // Walk rows within this column
            walk_rows_in_col(
                start_row,
                col_draw_y,
                height,
                rows,
                |_| row_height,
                freeze_trailing_rows,
                has_append_row,
                None,
                |draw_y, row, rh, is_sticky, is_trailing| {
                    let cell_x = col_draw_x;
                    let cell_y = draw_y;
                    let cell_w = col_width;
                    let cell_h = rh;

                    // Cell background
                    draw_cell_background(
                        ctx,
                        cell_x,
                        cell_y,
                        cell_w,
                        cell_h,
                        c.source_index,
                        row as i32,
                        selection,
                        is_sticky,
                        theme,
                    );

                    // Get cell data from JS callback
                    let cell = get_cell_content(c.source_index as i32, row as i32);

                    // Draw cell content
                    draw_cell_content(
                        ctx,
                        &cell,
                        cell_x,
                        cell_y,
                        cell_w,
                        cell_h,
                        c.source_index,
                        row as i32,
                        selection,
                        theme,
                        &font,
                    );

                    // Draw focus ring
                    if draw_focus && is_focused {
                        if cell_is_selected(c.source_index, row as i32, selection) {
                            ctx.set_stroke_style(&theme.accent_color);
                            ctx.set_line_width(2.0);
                            rounded_rect(ctx, cell_x, cell_y, cell_w, cell_h, theme.rounding_radius());
                            ctx.stroke();
                        }
                    }

                    false
                },
            );

            ctx.restore();
            false
        },
    );
}

fn draw_cell_background(
    ctx: &mut CanvasCtx,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    col: usize,
    row: i32,
    selection: &GridSelection,
    is_sticky: bool,
    theme: &Theme,
) {
    let is_selected = cell_is_selected(col, row, selection);
    let is_row_selected = selection.rows.contains(&row);
    let is_col_selected = selection.columns.contains(&(col as i32));

    // Determine background color
    let bg = if is_selected {
        // Selected cell: blend accent_light with bg_cell
        blend(&theme.accent_light, &theme.bg_cell)
    } else if is_row_selected || is_col_selected {
        theme.accent_light.clone()
    } else {
        theme.bg_cell.clone()
    };

    ctx.set_fill_style(&bg);
    ctx.fill_rect(x + 1.0, y, w - 1.0, h);
}

fn draw_cell_content(
    ctx: &mut CanvasCtx,
    cell: &GridCell,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    col: usize,
    row: i32,
    selection: &GridSelection,
    theme: &Theme,
    font: &str,
) {
    let rect = Rectangle::new(x, y, w, h);

    match cell {
        GridCell::Text { data, display_data, content_align } => {
            ctx.set_fill_style(&theme.text_dark);
            ctx.set_font(font);
            let display = display_data.as_deref().unwrap_or(data);
            draw_text_cell(ctx, display, &rect, theme, *content_align, font);
        }
        GridCell::Number { data: _, display_data, content_align } => {
            ctx.set_fill_style(&theme.text_dark);
            ctx.set_font(font);
            let empty = String::new();
            let display = display_data.as_deref().unwrap_or(&empty);
            // Numbers default to right-aligned
            let align = content_align.or(Some(ContentAlign::Right));
            draw_text_cell(ctx, display, &rect, theme, align, font);
        }
        GridCell::Loading { skeleton_width } => {
            draw_loading_cell(ctx, x, y, w, h, *skeleton_width, theme);
        }
    }
}

fn draw_loading_cell(
    ctx: &mut CanvasCtx,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    skeleton_width: Option<f64>,
    theme: &Theme,
) {
    let padding = theme.cell_horizontal_padding;
    let sw = skeleton_width.unwrap_or(w * 0.6).min(w - padding * 2.0);
    let sh = 8.0;
    let sx = x + padding;
    let sy = y + (h - sh) / 2.0;

    // Simple skeleton: a rounded rect filled with border color
    ctx.set_fill_style(&theme.border_color);
    rounded_rect(ctx, sx, sy, sw, sh, 4.0);
    ctx.fill();
}
