use crate::canvas::CanvasCtx;
use crate::columns::ResolvedColumns;
use crate::defaults::MIN_COLUMN_WIDTH;
use crate::layout::ColumnLayout;
use crate::theme::Theme;
use crate::types::{ColDragState, ConditionalRule, GridCell, GridSelection, ResizeState, SortState};
use crate::walk::MappedColumn;

use super::cells::draw_cells;

/// Fallback stroke color for resize indicator lines when theme provides no override.
const RESIZE_INDICATOR_COLOR: &str = "#000000";
use super::header::{draw_grid_headers, GroupDetails};
use super::lines::{draw_blanks, draw_grid_lines, draw_selection_ring};

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
    cell_y_offset: usize,
    translate_x: f64,
    translate_y: f64,
    freeze_trailing_rows: usize,
    has_append_row: bool,
    selection: &GridSelection,
    sort_state: &SortState,
    theme: &Theme,
    is_focused: bool,
    draw_focus: bool,
    get_cell_content: &dyn Fn(i32, i32) -> GridCell,
    get_group_details: &dyn Fn(&str) -> GroupDetails,
    resolved_columns: Option<&ResolvedColumns>,
    resize_state: Option<&ResizeState>,
    col_drag: Option<&ColDragState>,
    col_layout: &ColumnLayout,
    conditional_format_overrides: &std::collections::HashMap<String, Vec<ConditionalRule>>,
    column_stats: &std::collections::HashMap<String, (f64, f64)>,
    show_expand_icon_fn: &dyn Fn(usize, usize) -> bool,
    is_row_expanded: &dyn Fn(usize) -> bool,
    is_aggregate_row: &dyn Fn(usize) -> bool,
    is_group_key_col: &dyn Fn(usize) -> bool,
    is_depth_all_expanded: &dyn Fn(usize) -> bool,
) {
    let total_header_height = header_height + group_header_height;

    ctx.set_fill_style(&theme.bg_cell);
    ctx.fill_rect(0.0, 0.0, width, height);

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
        0,
        freeze_trailing_rows,
        theme,
        has_append_row,
    );

    draw_grid_headers(
        ctx,
        effective_cols,
        enable_groups,
        width,
        translate_x,
        header_height,
        group_header_height,
        selection,
        sort_state,
        theme,
        |_| true,
        get_group_details,
        resolved_columns,
        col_drag,
        col_layout,
        is_group_key_col,
        is_depth_all_expanded,
    );

    draw_cells(
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
        get_cell_content,
        selection,
        freeze_trailing_rows,
        has_append_row,
        theme,
        is_focused,
        draw_focus,
        resolved_columns,
        conditional_format_overrides,
        column_stats,
        show_expand_icon_fn,
        is_row_expanded,
        is_aggregate_row,
    );

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

    if let Some(rs) = resize_state {
        draw_resize_indicators(ctx, rs, effective_cols, translate_x, total_header_height, height);
    }
}

fn draw_resize_indicators(
    ctx: &CanvasCtx,
    rs: &ResizeState,
    effective_cols: &[MappedColumn],
    translate_x: f64,
    top: f64,
    bottom: f64,
) {
    let mut x = 0.0f64;
    let mut clip_x = 0.0f64;
    let mut left_x = None;

    for c in effective_cols {
        let draw_x = if c.sticky { clip_x } else { x + translate_x };
        if c.source_index == rs.column_display_index {
            left_x = Some(draw_x);
            break;
        }
        x += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }

    if let Some(lx) = left_x {
        let new_width = (rs.start_width + (rs.current_x - rs.start_x)).max(MIN_COLUMN_WIDTH);
        let right_x = lx + new_width;

        ctx.set_stroke_style(RESIZE_INDICATOR_COLOR);
        ctx.set_line_width(1.0);

        ctx.begin_path();
        ctx.move_to(lx + 0.5, top);
        ctx.line_to(lx + 0.5, bottom);
        ctx.stroke();

        ctx.begin_path();
        ctx.move_to(right_x + 0.5, top);
        ctx.line_to(right_x + 0.5, bottom);
        ctx.stroke();
    }
}
