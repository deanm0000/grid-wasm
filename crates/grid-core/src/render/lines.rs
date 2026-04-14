use crate::canvas::CanvasCtx;
use crate::theme::Theme;
use crate::types::{ContentAlign, GridCell, GridSelection, Rectangle};
use crate::walk::{walk_columns, walk_rows_in_col, MappedColumn};

use super::lib_utils::{cell_is_selected, draw_text_cell, rounded_rect};

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
        total_header_height + translate_y_to_f64(translate_y),
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

fn translate_y_to_f64(ty: f64) -> f64 {
    ty
}

/// Draw selection highlight ring(s) around selected cells.
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
    let total_header_height = header_height + group_header_height;

    ctx.save();
    ctx.clip_rect(0.0, total_header_height, width, height - total_header_height);

    let stroke_color = if is_focused { &theme.accent_color } else { &theme.text_medium };
    ctx.set_stroke_style(stroke_color);
    ctx.set_line_width(2.0);

    // Draw ring for the primary range
    if let Some(ref current) = selection.current {
        let range = &current.range;
        if let Some((rx, ry, rw, rh)) = compute_range_bounds(
            range.x as i32, range.y as i32,
            (range.x + range.width - 1.0) as i32,
            (range.y + range.height - 1.0) as i32,
            effective_cols, translate_x, translate_y,
            total_header_height, row_height, cell_y_offset,
        ) {
            rounded_rect(ctx, rx, ry, rw, rh, theme.rounding_radius());
            ctx.stroke();
        }
    }

    // Draw individual rings for ctrl_cells
    for item in &selection.ctrl_cells {
        if let Some((cx, cy, cw, ch)) = compute_cell_bounds(
            item.col as usize, item.row,
            effective_cols, translate_x, translate_y,
            total_header_height, row_height, cell_y_offset,
        ) {
            rounded_rect(ctx, cx, cy, cw, ch, theme.rounding_radius());
            ctx.stroke();
        }
    }

    ctx.restore();
}

fn find_col_x(
    col: usize,
    effective_cols: &[MappedColumn],
    translate_x: f64,
) -> Option<(f64, f64)> {
    let mut x_acc = 0.0f64;
    let mut clip_x = 0.0f64;
    for c in effective_cols {
        if c.source_index == col {
            let draw_x = if c.sticky { clip_x } else { x_acc + translate_x };
            return Some((draw_x, c.width));
        }
        x_acc += c.width;
        if c.sticky { clip_x += c.width; }
    }
    None
}

fn row_to_y(
    row: i32,
    translate_y: f64,
    total_header_height: f64,
    row_height: f64,
    cell_y_offset: usize,
) -> f64 {
    total_header_height + translate_y
        + ((row as isize - cell_y_offset as isize) as f64) * row_height
}

fn compute_cell_bounds(
    col: usize,
    row: i32,
    effective_cols: &[MappedColumn],
    translate_x: f64,
    translate_y: f64,
    total_header_height: f64,
    row_height: f64,
    cell_y_offset: usize,
) -> Option<(f64, f64, f64, f64)> {
    if row < 0 { return None; }
    let (cx, cw) = find_col_x(col, effective_cols, translate_x)?;
    let cy = row_to_y(row, translate_y, total_header_height, row_height, cell_y_offset);
    Some((cx, cy, cw, row_height))
}

fn compute_range_bounds(
    col1: i32, row1: i32, col2: i32, row2: i32,
    effective_cols: &[MappedColumn],
    translate_x: f64,
    translate_y: f64,
    total_header_height: f64,
    row_height: f64,
    cell_y_offset: usize,
) -> Option<(f64, f64, f64, f64)> {
    if row1 < 0 || row2 < 0 { return None; }

    let min_col = col1.min(col2) as usize;
    let max_col = col1.max(col2) as usize;
    let min_row = row1.min(row2);
    let max_row = row1.max(row2);

    // Find left edge of min_col
    let (x1, _) = find_col_x(min_col, effective_cols, translate_x)?;
    // Find right edge of max_col
    let (x2, w2) = find_col_x(max_col, effective_cols, translate_x)?;

    let y1 = row_to_y(min_row, translate_y, total_header_height, row_height, cell_y_offset);
    let y2 = row_to_y(max_row, translate_y, total_header_height, row_height, cell_y_offset);

    let rx = x1;
    let ry = y1;
    let rw = (x2 + w2) - x1;
    let rh = (y2 + row_height) - y1;

    Some((rx, ry, rw, rh))
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
    let clip_x: f64 = effective_cols.iter().filter(|c| c.sticky).map(|c| c.width).sum();
    let x_acc: f64 = effective_cols.iter().filter(|c| !c.sticky).map(|c| c.width).sum();
    let right_edge = clip_x + x_acc + translate_x;

    if right_edge >= width {
        return;
    }

    // Fill blank area
    ctx.set_fill_style(&theme.bg_cell);
    ctx.fill_rect(right_edge, total_header_height, width - right_edge, height - total_header_height);
}
