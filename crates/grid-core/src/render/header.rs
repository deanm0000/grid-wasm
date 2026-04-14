use crate::canvas::CanvasCtx;
use crate::columns::ResolvedColumns;
use crate::layout::{self, ColumnLayout, MENU_DOT_GAP, MENU_DOT_RADIUS};
use crate::theme::Theme;
use crate::types::{ColDragState, GridSelection, SortDirection, SortState};

use crate::walk::{walk_columns, MappedColumn};
use super::cells::{draw_expand_icon, EXPAND_ICON_PAD, EXPAND_ICON_SIZE};



/// Alpha (opacity) of the column header ghost while a column is being dragged.
const COL_DRAG_GHOST_ALPHA: f64 = 0.35;

pub fn draw_grid_headers(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    enable_groups: bool,
    width: f64,
    translate_x: f64,
    header_height: f64,
    group_header_height: f64,
    selection: &GridSelection,
    sort_state: &SortState,
    theme: &Theme,
    _vertical_border: impl Fn(usize) -> bool,
    _get_group_details: impl Fn(&str) -> GroupDetails,
    resolved: Option<&ResolvedColumns>,
    col_drag: Option<&ColDragState>,
    col_layout: &ColumnLayout,
    is_group_key_col: &dyn Fn(usize) -> bool,
    is_depth_all_expanded: &dyn Fn(usize) -> bool,
) {
    let total_header_height = header_height + group_header_height;
    if total_header_height <= 0.0 {
        return;
    }

    ctx.set_fill_style(&theme.bg_header);
    ctx.fill_rect(0.0, 0.0, width, total_header_height);

    let font = theme.header_font_full();
    ctx.set_font(&font);

    if let Some(resolved) = resolved {
        if resolved.max_depth > 1 {
            draw_multi_level_headers(
                ctx,
                effective_cols,
                resolved,
                width,
                translate_x,
                header_height,
                group_header_height,
                selection,
                sort_state,
                theme,
                &font,
                col_drag,
                col_layout,
                is_group_key_col,
                is_depth_all_expanded,
            );
            return;
        }
    }

    draw_leaf_headers(
        ctx, width, selection, sort_state, theme, &font, resolved, col_drag, col_layout,
        is_group_key_col, is_depth_all_expanded,
    );
}

fn draw_multi_level_headers(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    resolved: &ResolvedColumns,
    width: f64,
    translate_x: f64,
    header_height: f64,
    group_header_height: f64,
    selection: &GridSelection,
    sort_state: &SortState,
    theme: &Theme,
    font: &str,
    col_drag: Option<&ColDragState>,
    col_layout: &ColumnLayout,
    is_group_key_col: &dyn Fn(usize) -> bool,
    is_depth_all_expanded: &dyn Fn(usize) -> bool,
) {
    let total_header_height = header_height + group_header_height;
    let level_height = total_header_height / resolved.max_depth as f64;

    for (level_idx, spans) in resolved.header_levels.iter().enumerate() {
        let y = level_idx as f64 * level_height;
        let h = level_height;

        for span in spans {
            if span.title.is_empty() {
                continue;
            }

            let (span_x, span_w) =
                compute_span_bounds(span.first_leaf, span.last_leaf, effective_cols, translate_x);

            if span_x > width || span_w <= 0.0 {
                continue;
            }

            ctx.save();
            ctx.clip_rect(span_x, y, span_w, h);

            let bg = span
                .style
                .as_ref()
                .and_then(|s| s.bg_color.as_deref())
                .unwrap_or(&theme.bg_header);
            ctx.set_fill_style(bg);
            ctx.fill_rect(span_x, y, span_w, h);

            let text_color = span
                .style
                .as_ref()
                .and_then(|s| s.color.as_deref())
                .unwrap_or(&theme.text_header);
            let text_font = span
                .style
                .as_ref()
                .and_then(|s| s.font.as_deref())
                .unwrap_or(font);

            ctx.set_fill_style(text_color);
            ctx.set_font(text_font);
            ctx.set_text_align("left");
            ctx.set_text_baseline("middle");
            let text_x = span_x + theme.cell_horizontal_padding;
            let text_y = y + h / 2.0;
            let _ = ctx.fill_text(&span.title, text_x, text_y);
            ctx.set_text_baseline("alphabetic");

            ctx.restore();

            // Draw ⋮ button if this span has a menu entry
            if let Some(span_entry) = col_layout.span_menus.iter().find(|s| {
                s.level_idx == level_idx
                    && s.first_leaf == span.first_leaf
                    && s.last_leaf == span.last_leaf
            }) {
                ctx.set_fill_style(&theme.text_header);
                for i in -1i32..=1 {
                    ctx.begin_path();
                    let _ = ctx.arc(
                        span_entry.menu_btn_cx,
                        span_entry.menu_btn_cy + i as f64 * MENU_DOT_GAP,
                        MENU_DOT_RADIUS,
                        0.0,
                        std::f64::consts::TAU,
                    );
                    ctx.fill();
                }
            }

            ctx.set_stroke_style(&theme.border_color);
            ctx.set_line_width(1.0);
            ctx.begin_path();
            ctx.move_to(span_x + 0.5, y);
            ctx.line_to(span_x + 0.5, y + h);
            ctx.stroke();

            ctx.begin_path();
            ctx.move_to(span_x, y + h - 0.5);
            ctx.line_to(span_x + span_w, y + h - 0.5);
            ctx.stroke();
        }
    }

    draw_leaf_row_from_layout(
        ctx,
        width,
        selection,
        sort_state,
        theme,
        font,
        Some(resolved),
        col_drag,
        col_layout,
        is_group_key_col,
        is_depth_all_expanded,
    );
}

fn draw_leaf_headers(
    ctx: &mut CanvasCtx,
    width: f64,
    selection: &GridSelection,
    sort_state: &SortState,
    theme: &Theme,
    font: &str,
    resolved: Option<&ResolvedColumns>,
    col_drag: Option<&ColDragState>,
    col_layout: &ColumnLayout,
    is_group_key_col: &dyn Fn(usize) -> bool,
    is_depth_all_expanded: &dyn Fn(usize) -> bool,
) {
    draw_leaf_row_from_layout(
        ctx, width, selection, sort_state, theme, font, resolved, col_drag, col_layout,
        is_group_key_col, is_depth_all_expanded,
    );
}

fn draw_leaf_row_from_layout(
    ctx: &mut CanvasCtx,
    width: f64,
    selection: &GridSelection,
    sort_state: &SortState,
    theme: &Theme,
    font: &str,
    resolved: Option<&ResolvedColumns>,
    col_drag: Option<&ColDragState>,
    col_layout: &ColumnLayout,
    is_group_key_col: &dyn Fn(usize) -> bool,
    is_depth_all_expanded: &dyn Fn(usize) -> bool,
) {
    let y = col_layout.leaf_y;
    let h = col_layout.leaf_h;
    let tri_size = layout::tri_size();

    for entry in &col_layout.entries {
        let x = entry.draw_x;
        let w = entry.width;

        if x > width || w <= 0.0 {
            continue;
        }

        ctx.save();
        
        ctx.clip_rect(x, y, w, h);

        let is_being_dragged = col_drag.map_or(false, |d| {
            d.has_activated && d.col_display_index == entry.source_index
        });
        if is_being_dragged {
            ctx.set_global_alpha(COL_DRAG_GHOST_ALPHA);
        }

        let is_selected = selection.columns.contains(&(entry.source_index as i32));
        let has_selected_cell = selection
            .current
            .as_ref()
            .map_or(false, |s| s.cell.col == entry.source_index as i32);

        let leaf = resolved.and_then(|r| r.leaf_by_display_index(entry.source_index));

        let bg = if is_selected {
            &theme.accent_color
        } else if has_selected_cell {
            &theme.bg_header_has_focus
        } else {
            leaf.and_then(|l| l.header_style.as_ref())
                .and_then(|s| s.bg_color.as_deref())
                .unwrap_or(&theme.bg_header)
        };
        ctx.set_fill_style(bg);
        ctx.fill_rect(x, y, w, h);

        let text_color = if is_selected {
            &theme.text_header_selected
        } else {
            leaf.and_then(|l| l.header_style.as_ref())
                .and_then(|s| s.color.as_deref())
                .unwrap_or(&theme.text_header)
        };
        let text_font = leaf
            .and_then(|l| l.header_style.as_ref())
            .and_then(|s| s.font.as_deref())
            .unwrap_or(font);

        ctx.set_fill_style(text_color);
        ctx.set_font(text_font);
        ctx.set_text_align("left");
        ctx.set_text_baseline("middle");

        let right_reserved = layout::header_right_reserved_width();
        let is_gk = is_group_key_col(entry.source_index);
        let extra_reserved = if is_gk { EXPAND_ICON_SIZE + EXPAND_ICON_PAD * 2.0 } else { 0.0 };
        let text_x = x + theme.cell_horizontal_padding;
        let text_y = y + h / 2.0;
        let text_max = w - theme.cell_horizontal_padding * 2.0 - right_reserved - extra_reserved;

        let title = leaf.map(|l| l.display_name.as_str()).unwrap_or("");
        if text_max > 0.0 && !title.is_empty() {
            let _ = ctx.fill_text(title, text_x, text_y);
        }
        ctx.set_text_baseline("alphabetic");

        let is_sort_active = sort_state.column == Some(entry.source_index);

        let indicator_color = if is_selected {
            &theme.text_header_selected
        } else {
            &theme.text_header
        };
        ctx.set_fill_style(indicator_color);
        ctx.set_stroke_style(indicator_color);
        
        if is_sort_active {
            match sort_state.direction {
                Some(SortDirection::Ascending) => {
                    draw_triangle_up(ctx, entry.tri_up_cx, entry.tri_up_cy, tri_size, true);
                }
                Some(SortDirection::Descending) => {
                    draw_triangle_down(ctx, entry.tri_down_cx, entry.tri_down_cy, tri_size, true);
                }
                None => {
                    draw_triangle_up(ctx, entry.tri_up_cx, entry.tri_up_cy, tri_size, false);
                    draw_triangle_down(ctx, entry.tri_down_cx, entry.tri_down_cy, tri_size, false);
                }
            }
        } else {
            draw_triangle_up(ctx, entry.tri_up_cx, entry.tri_up_cy, tri_size, false);
            draw_triangle_down(ctx, entry.tri_down_cx, entry.tri_down_cy, tri_size, false);
        }

        // Draw 3-dot menu button (⋮)
        let dot_color = if is_selected {
            &theme.text_header_selected
        } else {
            &theme.text_header
        };
        ctx.set_fill_style(dot_color);
        for i in -1i32..=1 {
            ctx.begin_path();
            let _ = ctx.arc(
                entry.menu_btn_cx,
                entry.menu_btn_cy + i as f64 * MENU_DOT_GAP,
                MENU_DOT_RADIUS,
                0.0,
                std::f64::consts::TAU,
            );
            ctx.fill();
        }

        // Draw expand/collapse-all icon just to the left of the right-reserved area.
        if is_gk {
            let all_expanded = is_depth_all_expanded(entry.source_index);
            // Draw in a virtual cell whose right edge is at x + w - right_reserved.
            let virtual_w = w - right_reserved;
            draw_expand_icon(ctx, x, y, virtual_w, h, all_expanded, theme);
        }

        ctx.restore();
        
        if entry.source_index > 0 {
            ctx.set_stroke_style(&theme.border_color);
            ctx.set_line_width(1.0);
            ctx.begin_path();
            ctx.move_to(x + 0.5, y);
            ctx.line_to(x + 0.5, y + h);
            ctx.stroke();
        }
    }
}

fn compute_span_bounds(
    first_leaf: usize,
    last_leaf: usize,
    effective_cols: &[MappedColumn],
    translate_x: f64,
) -> (f64, f64) {
    let mut x = 0.0f64;
    let mut clip_x = 0.0f64;
    let mut span_x = None;
    let mut span_w = 0.0f64;

    for c in effective_cols {
        let draw_x = if c.sticky { clip_x } else { x + translate_x };
        if c.source_index >= first_leaf && c.source_index <= last_leaf {
            if span_x.is_none() {
                span_x = Some(draw_x);
            }
            span_w += c.width;
        }
        x += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }

    (span_x.unwrap_or(0.0), span_w)
}

fn draw_triangle_up(ctx: &CanvasCtx, cx: f64, cy: f64, size: f64, filled: bool) {
    ctx.begin_path();
    ctx.move_to(cx + size / 2.0, cy);
    ctx.line_to(cx + size, cy + size);
    ctx.line_to(cx, cy + size);
    ctx.close_path();
    if filled {
        ctx.fill();
    } else {
        ctx.set_line_width(1.0);
        ctx.stroke();
    }
}

fn draw_triangle_down(ctx: &CanvasCtx, cx: f64, cy: f64, size: f64, filled: bool) {
    ctx.begin_path();
    ctx.move_to(cx, cy);
    ctx.line_to(cx + size, cy);
    ctx.line_to(cx + size / 2.0, cy + size);
    ctx.close_path();
    if filled {
        ctx.fill();
    } else {
        ctx.set_line_width(1.0);
        ctx.stroke();
    }
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
