use crate::canvas::CanvasCtx;
use crate::theme::Theme;
use crate::types::{ContentAlign, GridCell, GridSelection, Rectangle};
use crate::walk::{walk_columns, walk_rows_in_col, MappedColumn};

use super::lib_utils::{cell_is_selected, draw_text_cell, get_middle_center_bias, rounded_rect};

/// Draw grid lines (horizontal and vertical borders).
pub fn draw_grid_lines(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    width: f64,
    height: f64,
    total_header_height: f64,
    translate_x: f64,
    translate_y: f64,
    cell_y_offset: usize,
    rows: usize,
    row_height: f64,
    freeze_columns: usize,
    freeze_trailing_rows: usize,
    theme: &Theme,
    has_append_row: bool,
) {
    // Draw vertical lines between columns
    ctx.set_stroke_style(&theme.border_color);
    ctx.set_line_width(1.0);
    ctx.set_line_cap("butt");

    walk_columns(
        effective_cols,
        cell_y_offset,
        translate_x,
        translate_y,
        total_header_height,
        |c, draw_x, _draw_y, clip_x, _start_row| {
            let col_x = if c.sticky { draw_x } else { draw_x + 0.5 };

            // Vertical line at right edge of column
            if col_x > 0.0 && col_x < width {
                ctx.begin_path();
                ctx.move_to(col_x, 0.0);
                ctx.line_to(col_x, height);
                ctx.stroke();
            }

            false
        },
    );

    // Draw horizontal lines between rows
    walk_rows_in_col(
        cell_y_offset,
        total_header_height + translateY_to_f64(translate_y),
        height,
        rows,
        |_| row_height,
        freeze_trailing_rows,
        has_append_row,
        None,
        |draw_y, _row, rh, _is_sticky, _is_trailing| {
            let line_y = draw_y + rh + 0.5;
            if line_y < height {
                ctx.begin_path();
                ctx.move_to(0.0, line_y);
                ctx.line_to(width, line_y);
                ctx.stroke();
            }
            false
        },
    );

    // Draw header bottom border
    if total_header_height > 0.0 {
        let border_color = theme.header_bottom_border_color.as_deref().unwrap_or(&theme.border_color);
        ctx.set_stroke_style(border_color);
        ctx.begin_path();
        ctx.move_to(0.0, total_header_height + 0.5);
        ctx.line_to(width, total_header_height + 0.5);
        ctx.stroke();
    }
}

fn translateY_to_f64(ty: f64) -> f64 {
    ty
}

/// Draw selection highlight ring around the current cell.
pub fn draw_selection_ring(
    ctx: &mut CanvasCtx,
    selection: &GridSelection,
    effective_cols: &[MappedColumn],
    width: f64,
    height: f64,
    header_height: f64,
    group_header_height: f64,
    rows: usize,
    row_height: f64,
    cell_y_offset: usize,
    translate_x: f64,
    translate_y: f64,
    freeze_trailing_rows: usize,
    theme: &Theme,
    is_focused: bool,
) {
    if let Some(ref current) = selection.current {
        let col = current.cell.col as usize;
        let row = current.cell.row;

        // Calculate bounds
        let total_header_height = header_height + group_header_height;

        // Find x
        let mut x_acc: f64 = 0.0;
        let mut clip_x: f64 = 0.0;
        let mut cell_x = 0.0f64;
        let mut cell_w = 0.0f64;

        for c in effective_cols {
            if c.source_index == col {
                cell_x = if c.sticky { clip_x } else { x_acc + translate_x };
                cell_w = c.width;
                break;
            }
            x_acc += c.width;
            if c.sticky {
                clip_x += c.width;
            }
        }

        // Find y
        let cell_y = if row == -1 {
            group_header_height
        } else if row >= 0 {
            let row_idx = row as usize;
            total_header_height + translate_y + ((row_idx as isize - cell_y_offset as isize) as f64) * row_height
        } else {
            return;
        };

        let cell_h = if row == -1 { header_height - group_header_height } else { row_height };

        // Clip to visible area
        ctx.save();
        ctx.clip_rect(0.0, total_header_height, width, height - total_header_height);

        // Draw focus ring
        if is_focused {
            ctx.set_stroke_style(&theme.accent_color);
        } else {
            ctx.set_stroke_style(&theme.text_medium);
        }
        ctx.set_line_width(2.0);

        rounded_rect(ctx, cell_x, cell_y, cell_w, cell_h, theme.rounding_radius());
        ctx.stroke();

        ctx.restore();
    }
}

/// Draw background for blank areas to the right of data and for selected/disabled rows.
pub fn draw_blanks(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    all_cols: &[MappedColumn],
    width: f64,
    height: f64,
    total_header_height: f64,
    translate_x: f64,
    translate_y: f64,
    cell_y_offset: usize,
    rows: usize,
    row_height: f64,
    selected_rows: &[i32],
    freeze_trailing_rows: usize,
    has_append_row: bool,
    theme: &Theme,
) {
    // Find the rightmost column edge
    let mut right_edge = 0.0f64;
    let mut clip_x = 0.0f64;
    for c in effective_cols {
        if c.sticky {
            clip_x += c.width;
        }
    }
    right_edge = clip_x;
    let mut x_acc = 0.0f64;
    for c in effective_cols {
        if !c.sticky {
            x_acc += c.width;
        }
    }
    right_edge += x_acc + translate_x;

    if right_edge >= width {
        return;
    }

    // Fill blank area
    ctx.set_fill_style(&theme.bg_cell);
    ctx.fill_rect(right_edge, total_header_height, width - right_edge, height - total_header_height);
}
