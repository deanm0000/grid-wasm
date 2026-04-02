use crate::canvas::CanvasCtx;
use crate::columns::ResolvedColumns;
use crate::theme::Theme;
use crate::types::{ColDragState, GridSelection, Rectangle, SortDirection, SortState};
use crate::walk::{walk_columns, MappedColumn};

use super::lib_utils::get_middle_center_bias;

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
) {
    let total_header_height = header_height + group_header_height;
    if total_header_height <= 0.0 {
        return;
    }

    ctx.set_fill_style(&theme.bg_header);
    ctx.fill_rect(0.0, 0.0, width, total_header_height);

    let font = theme.header_font_full();
    let base_font = theme.base_font_full();
    ctx.set_font(&font);
    let bias = get_middle_center_bias(ctx, &base_font);

    if let Some(resolved) = resolved {
        if resolved.max_depth > 1 {
            draw_multi_level_headers(
                ctx, effective_cols, resolved, width, translate_x,
                header_height, group_header_height, selection, sort_state,
                theme, &font, bias, col_drag,
            );
            return;
        }
    }

    draw_leaf_headers(
        ctx, effective_cols, enable_groups, width, translate_x,
        header_height, group_header_height, selection, sort_state,
        theme, &font, bias, resolved, col_drag,
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
    bias: f64,
    col_drag: Option<&ColDragState>,
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

            let (span_x, span_w) = compute_span_bounds(
                span.first_leaf, span.last_leaf, effective_cols, translate_x,
            );

            if span_x > width || span_w <= 0.0 {
                continue;
            }

            ctx.save();
            ctx.clip_rect(span_x, y, span_w, h);

            let bg = span.style.as_ref()
                .and_then(|s| s.bg_color.as_deref())
                .unwrap_or(&theme.bg_header);
            ctx.set_fill_style(bg);
            ctx.fill_rect(span_x, y, span_w, h);

            let text_color = span.style.as_ref()
                .and_then(|s| s.color.as_deref())
                .unwrap_or(&theme.text_header);
            let text_font = span.style.as_ref()
                .and_then(|s| s.font.as_deref())
                .unwrap_or(font);

            ctx.set_fill_style(text_color);
            ctx.set_font(text_font);
            ctx.set_text_align("left");
            let text_x = span_x + theme.cell_horizontal_padding;
            let text_y = y + h / 2.0 + bias;
            let _ = ctx.fill_text(&span.title, text_x, text_y);

            ctx.restore();

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

    let leaf_y = (resolved.max_depth - 1) as f64 * level_height;
    let leaf_h = level_height;

    draw_leaf_row(
        ctx, effective_cols, width, translate_x, leaf_y, leaf_h,
        selection, sort_state, theme, font, bias, Some(resolved),
        col_drag,
    );
}

fn draw_leaf_headers(
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
    font: &str,
    bias: f64,
    resolved: Option<&ResolvedColumns>,
    col_drag: Option<&ColDragState>,
) {
    let y = if enable_groups { group_header_height } else { 0.0 };
    let h = header_height - y;

    draw_leaf_row(
        ctx, effective_cols, width, translate_x, y, h,
        selection, sort_state, theme, font, bias, resolved,
        col_drag,
    );
}

fn draw_leaf_row(
    ctx: &mut CanvasCtx,
    effective_cols: &[MappedColumn],
    width: f64,
    translate_x: f64,
    y: f64,
    h: f64,
    selection: &GridSelection,
    sort_state: &SortState,
    theme: &Theme,
    font: &str,
    bias: f64,
    resolved: Option<&ResolvedColumns>,
    col_drag: Option<&ColDragState>,
) {
    walk_columns(
        effective_cols,
        0,
        translate_x,
        0.0,
        y + h,
        |c, draw_x, _draw_y, clip_x, _start_row| {
            let diff = if clip_x > draw_x { clip_x - draw_x } else { 0.0 };
            let x = draw_x + diff;
            let w = c.width - diff;

            if x > width || w <= 0.0 {
                return false;
            }

            ctx.save();
            ctx.clip_rect(x, y, w, h);

            let is_being_dragged = col_drag
                .map_or(false, |d| d.has_activated && d.col_display_index == c.source_index);
            if is_being_dragged {
                ctx.set_global_alpha(0.35);
            }

            let is_selected = selection.columns.contains(&(c.source_index as i32));
            let has_selected_cell = selection.current.as_ref()
                .map_or(false, |s| s.cell.col == c.source_index as i32);

            let leaf = resolved.and_then(|r| r.leaf_by_display_index(c.source_index));

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
            let text_font = leaf.and_then(|l| l.header_style.as_ref())
                .and_then(|s| s.font.as_deref())
                .unwrap_or(font);

            ctx.set_fill_style(text_color);
            ctx.set_font(text_font);
            ctx.set_text_align("left");

            let tri_area = 16.0;
            let text_x = x + theme.cell_horizontal_padding;
            let text_y = y + h / 2.0 + bias;
            let text_max = w - theme.cell_horizontal_padding * 2.0 - tri_area;
            if text_max > 0.0 {
                let _ = ctx.fill_text(&c.title, text_x, text_y);
            }

            let is_sort_active = sort_state.column == Some(c.source_index);
            let tri_padding = 6.0;
            let tri_size = 7.0;
            let tri_gap = 3.0;
            let tri_cx = x + w - tri_padding - tri_size;
            let tri_center_y = y + h / 2.0;
            let up_cy = tri_center_y - tri_size / 2.0 - tri_gap / 2.0;
            let down_cy = tri_center_y + tri_size / 2.0 + tri_gap / 2.0;

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
                        draw_triangle_up(ctx, tri_cx, up_cy, tri_size, true);
                    }
                    Some(SortDirection::Descending) => {
                        draw_triangle_down(ctx, tri_cx, down_cy, tri_size, true);
                    }
                    None => {
                        draw_triangle_up(ctx, tri_cx, up_cy, tri_size, false);
                        draw_triangle_down(ctx, tri_cx, down_cy, tri_size, false);
                    }
                }
            } else {
                draw_triangle_up(ctx, tri_cx, up_cy, tri_size, false);
                draw_triangle_down(ctx, tri_cx, down_cy, tri_size, false);
            }

            ctx.restore();

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

/// Compute the leaf row y and h using the same geometry as the renderers.
pub fn leaf_row_geometry(
    header_height: f64,
    group_header_height: f64,
    resolved: Option<&ResolvedColumns>,
) -> (f64, f64) {
    if let Some(r) = resolved {
        if r.max_depth > 1 {
            let total = header_height + group_header_height;
            let level_h = total / r.max_depth as f64;
            let leaf_y = (r.max_depth - 1) as f64 * level_h;
            return (leaf_y, level_h);
        }
    }
    // Single level: leaf row starts at group_header_height
    let y = group_header_height;
    let h = header_height - group_header_height;
    (y, h)
}

pub fn hit_test_sort_triangle(
    px: f64,
    py: f64,
    col: usize,
    effective_cols: &[MappedColumn],
    header_height: f64,
    group_header_height: f64,
    translate_x: f64,
    resolved: Option<&ResolvedColumns>,
) -> Option<bool> {
    let mut x: f64 = 0.0;
    let mut clip_x: f64 = 0.0;
    let mut col_x = None;
    let mut col_w = 0.0f64;

    for c in effective_cols {
        let draw_x = if c.sticky { clip_x } else { x + translate_x };
        if c.source_index == col {
            col_x = Some(draw_x);
            col_w = c.width;
            break;
        }
        x += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }

    let col_x = col_x?;
    let (y, h) = leaf_row_geometry(header_height, group_header_height, resolved);

    let tri_padding = 6.0;
    let tri_size = 7.0;
    let tri_gap = 3.0;
    let tri_cx = col_x + col_w - tri_padding - tri_size;
    let tri_center_y = y + h / 2.0;
    let up_cy = tri_center_y - tri_size / 2.0 - tri_gap / 2.0;
    let down_cy = tri_center_y + tri_size / 2.0 + tri_gap / 2.0;

    let hit_pad = 3.0;
    let up_rect = Rectangle::new(tri_cx - hit_pad, up_cy - hit_pad, tri_size + hit_pad * 2.0, tri_size + hit_pad * 2.0);
    let down_rect = Rectangle::new(tri_cx - hit_pad, down_cy - hit_pad, tri_size + hit_pad * 2.0, tri_size + hit_pad * 2.0);

    if up_rect.contains(px, py) {
        Some(true)
    } else if down_rect.contains(px, py) {
        Some(false)
    } else {
        None
    }
}

pub fn hit_test_resize_border(
    px: f64,
    py: f64,
    effective_cols: &[MappedColumn],
    header_height: f64,
    group_header_height: f64,
    translate_x: f64,
    buffer: f64,
    resolved: Option<&ResolvedColumns>,
) -> Option<usize> {
    let total = header_height + group_header_height;
    if py > total {
        return None;
    }
    // Only detect resize in the leaf header row
    let (leaf_y, leaf_h) = leaf_row_geometry(header_height, group_header_height, resolved);
    if py < leaf_y || py > leaf_y + leaf_h {
        return None;
    }

    let mut x = 0.0f64;
    let mut clip_x = 0.0f64;

    for c in effective_cols {
        let draw_x = if c.sticky { clip_x } else { x + translate_x };
        let right_edge = draw_x + c.width;

        if (px - right_edge).abs() <= buffer && c.is_resizable {
            return Some(c.source_index);
        }

        x += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }

    None
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
