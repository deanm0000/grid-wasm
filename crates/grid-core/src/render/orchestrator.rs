use crate::canvas::CanvasCtx;
use crate::theme::Theme;
use crate::types::{GridCell, GridSelection, Rectangle};
use crate::walk::MappedColumn;

use super::cells::draw_cells;
use super::header::{draw_grid_headers, GroupDetails};
use super::lines::{draw_blanks, draw_grid_lines, draw_selection_ring};

/// Main render function. Orchestrates all drawing operations.
pub fn draw_grid(
    ctx: &mut CanvasCtx,
    width: f64,
    height: f64,
    effective_cols: &[MappedColumn],
    all_cols: &[MappedColumn],
    rows: usize,
    row_height: f64,
    header_height: f64,
    group_header_height: f64,
    enable_groups: bool,
    cell_x_offset: usize,
    cell_y_offset: usize,
    translate_x: f64,
    translate_y: f64,
    freeze_columns: usize,
    freeze_trailing_rows: usize,
    has_append_row: bool,
    selection: &GridSelection,
    theme: &Theme,
    is_focused: bool,
    draw_focus: bool,
    get_cell_content: &dyn Fn(i32, i32) -> GridCell,
    get_group_details: &dyn Fn(&str) -> GroupDetails,
    vertical_border: &dyn Fn(usize) -> bool,
) {
    let total_header_height = header_height + group_header_height;

    // 1. Clear and fill background
    ctx.set_fill_style(&theme.bg_cell);
    ctx.fill_rect(0.0, 0.0, width, height);

    // 2. Draw grid lines
    draw_grid_lines(
        ctx,
        effective_cols,
        width,
        height,
        total_header_height,
        translate_x,
        translate_y,
        cell_y_offset,
        rows,
        row_height,
        freeze_columns,
        freeze_trailing_rows,
        theme,
        has_append_row,
    );

    // 3. Draw headers
    draw_grid_headers(
        ctx,
        effective_cols,
        enable_groups,
        width,
        translate_x,
        header_height,
        group_header_height,
        selection,
        theme,
        vertical_border,
        get_group_details,
    );

    // 4. Draw cells
    draw_cells(
        ctx,
        effective_cols,
        height,
        total_header_height,
        translate_x,
        translate_y,
        cell_y_offset,
        rows,
        row_height,
        get_cell_content,
        selection,
        freeze_trailing_rows,
        has_append_row,
        theme,
        is_focused,
        draw_focus,
    );

    // 5. Draw blanks (area to the right of data)
    draw_blanks(
        ctx,
        effective_cols,
        all_cols,
        width,
        height,
        total_header_height,
        translate_x,
        translate_y,
        cell_y_offset,
        rows,
        row_height,
        &[],
        freeze_trailing_rows,
        has_append_row,
        theme,
    );

    // 6. Draw selection ring
    if selection.current.is_some() {
        draw_selection_ring(
            ctx,
            selection,
            effective_cols,
            width,
            height,
            header_height,
            group_header_height,
            rows,
            row_height,
            cell_y_offset,
            translate_x,
            translate_y,
            freeze_trailing_rows,
            theme,
            is_focused,
        );
    }
}
