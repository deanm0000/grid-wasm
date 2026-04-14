use crate::canvas::CanvasCtx;
use crate::color::{blend, interpolate_colors};
use crate::columns::ResolvedColumns;
use crate::number_format::{format_accounting_parts, format_number, is_accounting};
use crate::theme::Theme;
use crate::types::{
    CellStyleOverride, ConditionalRule, ContentAlign, DataStyle, GridCell, GridSelection, Rectangle,
};
use crate::walk::{walk_columns, walk_rows_in_col, MappedColumn};

use super::lib_utils::{cell_is_selected, draw_text_cell, get_middle_center_bias, rounded_rect};

/// Height of the skeleton loading placeholder bar in pixels.
const SKELETON_HEIGHT: f64 = 8.0;
/// Corner radius of the skeleton loading bar.
const SKELETON_CORNER_RADIUS: f64 = 4.0;
/// Default skeleton bar width as a fraction of the cell width.
const SKELETON_WIDTH_RATIO: f64 = 0.6;
/// Stroke width for the cell selection focus ring.
const FOCUS_RING_WIDTH: f64 = 2.0;
/// 1 px left inset on cell background fill to leave room for the vertical grid line.
const CELL_BACKGROUND_LEFT_INSET: f64 = 1.0;
/// Width of the expand +/- icon box.
const EXPAND_ICON_SIZE: f64 = 14.0;
/// Gap between expand icon and right cell edge.
const EXPAND_ICON_PAD: f64 = 6.0;

pub fn draw_cells(
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
    get_cell_content: &dyn Fn(i32, i32) -> GridCell,
    selection: &GridSelection,
    freeze_trailing_rows: usize,
    has_append_row: bool,
    theme: &Theme,
    is_focused: bool,
    draw_focus: bool,
    resolved: Option<&ResolvedColumns>,
    conditional_format_overrides: &std::collections::HashMap<String, Vec<ConditionalRule>>,
    column_stats: &std::collections::HashMap<String, (f64, f64)>,
    show_expand_icon_fn: &dyn Fn(usize, usize) -> bool,
    is_row_expanded: &dyn Fn(usize) -> bool,
    is_aggregate_row: &dyn Fn(usize) -> bool,
) {
    let font = theme.base_font_full();
    let bold_font = format!("700 {}", font);

    let col_width_by_source: std::collections::HashMap<usize, f64> = effective_cols.iter()
        .map(|c| (c.source_index, c.width))
        .collect();

    walk_columns(
        effective_cols,
        cell_y_offset,
        translate_x,
        translate_y,
        total_header_height,
        |c, draw_x, _col_draw_y, clip_x, start_row| {
            let diff = if clip_x > draw_x { clip_x - draw_x } else { 0.0 };
            let col_draw_x = draw_x + diff;
            let col_width = c.width - diff;
            let col_draw_y = total_header_height + CELL_BACKGROUND_LEFT_INSET;
            let col_height = height - total_header_height - CELL_BACKGROUND_LEFT_INSET;

            if col_draw_x >= width || col_width <= 0.0 {
                return false;
            }

            let leaf = resolved.and_then(|r| r.leaf_by_display_index(c.source_index));
            let data_style = leaf.and_then(|l| l.data_style.as_ref());
            let arrow_name = leaf.map(|l| l.arrow_name.as_str()).unwrap_or("");

            walk_rows_in_col(
                start_row,
                col_draw_y,
                height,
                rows,
                |_| row_height,
                freeze_trailing_rows,
                has_append_row,
                None,
                |draw_y, row, rh, _is_sticky, _is_trailing| {
                    let cell = get_cell_content(c.source_index as i32, row as i32);

                    // Skip cells that are merged into a sibling to the left.
                    if matches!(cell, GridCell::Skip { .. }) {
                        return false;
                    }

                    let is_agg = is_aggregate_row(row as usize);
                    let row_font = if is_agg { bold_font.as_str() } else { &font };

                    let show_expand_icon = is_agg && show_expand_icon_fn(c.source_index, row as usize);

                    // For merge-start cells, compute the extended draw width covering sibling cols.
                    let (cell_x, cell_y, cell_w, cell_h) = if let GridCell::Skip { .. } = get_cell_content(c.source_index as i32 + 1, row as i32) {
                        let mut extra = 0.0f64;
                        let mut next_si = c.source_index + 1;
                        loop {
                            match get_cell_content(next_si as i32, row as i32) {
                                GridCell::Skip { .. } => {
                                    extra += col_width_by_source.get(&next_si).copied().unwrap_or(0.0);
                                    next_si += 1;
                                }
                                _ => break,
                            }
                        }
                        (col_draw_x, draw_y, col_width + extra, rh)
                    } else {
                        (col_draw_x, draw_y, col_width, rh)
                    };

                    // Shrink content rect rightward to leave room for the expand icon.
                    let content_w = if show_expand_icon {
                        (cell_w - EXPAND_ICON_SIZE - EXPAND_ICON_PAD * 2.0).max(0.0)
                    } else {
                        cell_w
                    };

                    ctx.save();
                    ctx.clip_rect(cell_x, col_draw_y, cell_w, col_height);

                    let min_max = column_stats.get(arrow_name).copied();
                    let cond_style = if let Some(override_rules) = conditional_format_overrides.get(arrow_name) {
                        evaluate_conditions(override_rules, &cell, min_max)
                    } else {
                        data_style
                            .and_then(|ds| ds.conditional_formats.as_ref())
                            .and_then(|rules| evaluate_conditions(rules, &cell, min_max))
                    };

                    draw_cell_background(
                        ctx, cell_x, cell_y, cell_w, cell_h,
                        c.source_index, row as i32, selection,
                        theme, data_style, cond_style.as_ref(),
                    );

                    draw_cell_content(
                        ctx, &cell, cell_x, cell_y, content_w, cell_h,
                        theme, row_font, data_style, cond_style.as_ref(),
                    );

                    if show_expand_icon {
                        let expanded = is_row_expanded(row as usize);
                        draw_expand_icon(ctx, cell_x, cell_y, cell_w, cell_h, expanded, theme);
                    }

                    // Thick bottom border for aggregate rows to visually separate them.
                    if is_agg {
                        let border_y = (cell_y + cell_h - 1.0).floor() + 0.5;
                        ctx.set_stroke_style(theme.horizontal_border_color());
                        ctx.set_line_width(2.0);
                        ctx.begin_path();
                        ctx.move_to(cell_x, border_y);
                        ctx.line_to(cell_x + cell_w, border_y);
                        ctx.stroke();
                    }

                    if draw_focus && is_focused {
                        if cell_is_selected(c.source_index, row as i32, selection) {
                            ctx.set_stroke_style(&theme.accent_color);
                            ctx.set_line_width(FOCUS_RING_WIDTH);
                            rounded_rect(ctx, cell_x, cell_y, cell_w, cell_h, theme.rounding_radius());
                            ctx.stroke();
                        }
                    }

                    ctx.restore();
                    false
                },
            );

            false
        },
    );
}

/// Draw the +/- expand icon at the right edge of a group-key cell.
fn draw_expand_icon(
    ctx: &mut CanvasCtx,
    cell_x: f64,
    cell_y: f64,
    cell_w: f64,
    cell_h: f64,
    expanded: bool,
    theme: &Theme,
) {
    let size = EXPAND_ICON_SIZE;
    let pad = EXPAND_ICON_PAD;
    let rx = cell_x + cell_w - pad - size;
    let ry = cell_y + (cell_h - size) / 2.0;

    ctx.set_stroke_style(&theme.text_medium);
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.move_to(rx, ry);
    ctx.line_to(rx + size, ry);
    ctx.line_to(rx + size, ry + size);
    ctx.line_to(rx, ry + size);
    ctx.close_path();
    ctx.stroke();

    let cx = rx + size / 2.0;
    let cy = ry + size / 2.0;
    let arm = size * 0.28;

    ctx.set_stroke_style(&theme.text_medium);
    ctx.set_line_width(1.5);

    ctx.begin_path();
    ctx.move_to(cx - arm, cy);
    ctx.line_to(cx + arm, cy);
    ctx.stroke();

    if !expanded {
        ctx.begin_path();
        ctx.move_to(cx, cy - arm);
        ctx.line_to(cx, cy + arm);
        ctx.stroke();
    }
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
    theme: &Theme,
    data_style: Option<&DataStyle>,
    cond_style: Option<&CellStyleOverride>,
) {
    let is_selected = cell_is_selected(col, row, selection);
    let is_row_selected = selection.rows.contains(&row);
    let is_col_selected = selection.columns.contains(&(col as i32));

    let base_bg = cond_style
        .and_then(|s| s.bg_color.as_deref())
        .or_else(|| data_style.and_then(|s| s.bg_color.as_deref()));

    let bg = if is_selected {
        blend(&theme.accent_light, base_bg.unwrap_or(&theme.bg_cell))
    } else if is_row_selected || is_col_selected {
        theme.accent_light.clone()
    } else {
        base_bg.unwrap_or(&theme.bg_cell).to_string()
    };

    ctx.set_fill_style(&bg);
    ctx.fill_rect(x + CELL_BACKGROUND_LEFT_INSET, y, w - CELL_BACKGROUND_LEFT_INSET, h);
}

fn draw_cell_content(
    ctx: &mut CanvasCtx,
    cell: &GridCell,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    theme: &Theme,
    default_font: &str,
    data_style: Option<&DataStyle>,
    cond_style: Option<&CellStyleOverride>,
) {
    let rect = Rectangle::new(x, y, w, h);

    let color = cond_style
        .and_then(|s| s.color.as_deref())
        .or_else(|| data_style.and_then(|s| s.color.as_deref()))
        .unwrap_or(&theme.text_dark);
    let font = cond_style
        .and_then(|s| s.font.as_deref())
        .or_else(|| data_style.and_then(|s| s.font.as_deref()))
        .unwrap_or(default_font);
    let style_align = data_style.and_then(|s| s.align);
    let num_format = data_style.and_then(|s| s.number_format.as_ref());

    match cell {
        GridCell::Text { data, display_data, content_align } => {
            ctx.set_fill_style(color);
            ctx.set_font(font);

            // If a Date/DateTime format is set on a text cell, attempt to parse
            // the raw string as an ISO 8601 datetime and reformat it.
            let reformatted;
            let display = if let Some(fmt) = num_format {
                if matches!(fmt, crate::types::NumberFormat::Date { .. } | crate::types::NumberFormat::DateTime { .. }) {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(data) {
                        reformatted = format_number(dt.timestamp_micros() as f64, fmt);
                        reformatted.as_str()
                    } else {
                        display_data.as_deref().unwrap_or(data)
                    }
                } else {
                    display_data.as_deref().unwrap_or(data)
                }
            } else {
                display_data.as_deref().unwrap_or(data)
            };

            let align = style_align.or(*content_align);
            draw_text_cell(ctx, display, &rect, theme, align, font);
        }
        GridCell::Number { data, display_data, content_align } => {
            ctx.set_fill_style(color);
            ctx.set_font(font);

            // Accounting format: draw symbol pinned left, number right-aligned
            if let (Some(val), Some(fmt)) = (data, num_format) {
                if let Some(decimals) = is_accounting(fmt) {
                    let (symbol, number_str) = format_accounting_parts(*val, decimals);
                    draw_accounting_cell(ctx, symbol, &number_str, &rect, theme, font, color);
                    return;
                }
            }

            let formatted = if let (Some(val), Some(fmt)) = (data, num_format) {
                Some(format_number(*val, fmt))
            } else {
                None
            };
            let display = formatted
                .as_deref()
                .or(display_data.as_deref())
                .unwrap_or("");

            let align = style_align
                .or(*content_align)
                .or(Some(ContentAlign::Right));
            draw_text_cell(ctx, display, &rect, theme, align, font);
        }
        GridCell::Loading { skeleton_width } => {
            draw_loading_cell(ctx, x, y, w, h, *skeleton_width, theme);
        }
        GridCell::Skip { .. } => {}
    }
}

/// Draw accounting format: symbol pinned to left padding, number right-aligned.
/// e.g.:  $         1,234.56
///        $        (1,234.56)
fn draw_accounting_cell(
    ctx: &mut CanvasCtx,
    symbol: &str,
    number_str: &str,
    rect: &Rectangle,
    theme: &Theme,
    font: &str,
    color: &str,
) {
    use super::lib_utils::get_middle_center_bias;

    let bias = get_middle_center_bias(ctx, font);
    let center_y = rect.y + rect.height / 2.0 + bias;
    let pad = theme.cell_horizontal_padding;

    ctx.set_font(font);
    ctx.set_fill_style(color);

    let symbol_w = ctx.measure_text_width(symbol, font);
    let number_w = ctx.measure_text_width(number_str, font);
    let available = rect.width - 2.0 * pad;

    if symbol_w + number_w <= available {
        ctx.set_text_align("left");
        let _ = ctx.fill_text(symbol, rect.x + pad, center_y);
    }

    // Number: right-aligned at right edge minus padding
    ctx.set_text_align("right");
    let _ = ctx.fill_text(number_str, rect.x + rect.width - pad, center_y);
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
    let sw = skeleton_width.unwrap_or(w * SKELETON_WIDTH_RATIO).min(w - padding * 2.0);
    let sh = SKELETON_HEIGHT;
    let sx = x + padding;
    let sy = y + (h - sh) / 2.0;
    ctx.set_fill_style(&theme.border_color);
    rounded_rect(ctx, sx, sy, sw, sh, SKELETON_CORNER_RADIUS);
    ctx.fill();
}

fn evaluate_conditions(
    rules: &[ConditionalRule],
    cell: &GridCell,
    min_max: Option<(f64, f64)>,
) -> Option<CellStyleOverride> {
    let numeric_val = match cell {
        GridCell::Number { data: Some(v), .. } => Some(*v),
        _ => None,
    };
    let string_val = match cell {
        GridCell::Text { data, display_data, .. } => {
            Some(display_data.as_deref().unwrap_or(data.as_str()))
        }
        _ => None,
    };
    let is_null = match cell {
        GridCell::Text { data, .. } => data.is_empty(),
        GridCell::Number { data: None, .. } => true,
        _ => false,
    };

    for rule in rules {
        match rule {
            ConditionalRule::GreaterThan { value, style } => {
                if numeric_val.map_or(false, |v| v > *value) {
                    return Some(style.clone());
                }
            }
            ConditionalRule::LessThan { value, style } => {
                if numeric_val.map_or(false, |v| v < *value) {
                    return Some(style.clone());
                }
            }
            ConditionalRule::Equal { value, style } => {
                if numeric_val.map_or(false, |v| (v - value).abs() < f64::EPSILON) {
                    return Some(style.clone());
                }
            }
            ConditionalRule::Between { min, max, style } => {
                if numeric_val.map_or(false, |v| v >= *min && v <= *max) {
                    return Some(style.clone());
                }
            }
            ConditionalRule::Contains { value, style } => {
                if string_val.map_or(false, |s| s.contains(value.as_str())) {
                    return Some(style.clone());
                }
            }
            ConditionalRule::IsNull { style } => {
                if is_null { return Some(style.clone()); }
            }
            ConditionalRule::IsNotNull { style } => {
                if !is_null { return Some(style.clone()); }
            }
            ConditionalRule::Percentile { .. } => {}
            ConditionalRule::Gradient { low_color, high_color, min_value, max_value } => {
                if let Some(v) = numeric_val {
                    // Use explicit overrides if provided, otherwise fall back to column stats
                    let effective_min = min_value.or_else(|| min_max.map(|(mn, _)| mn));
                    let effective_max = max_value.or_else(|| min_max.map(|(_, mx)| mx));
                    if let (Some(min), Some(max)) = (effective_min, effective_max) {
                        let range = max - min;
                        let t = if range > 0.0 { ((v - min) / range).clamp(0.0, 1.0) as f32 } else { 0.5 };
                        let color = interpolate_colors(low_color, high_color, t);
                        return Some(CellStyleOverride { bg_color: Some(color), color: None, font: None });
                    }
                }
            }
            ConditionalRule::ValueColor { rules: value_rules } => {
                if let Some(display) = string_val {
                    for vr in value_rules {
                        if display == vr.value.as_str() {
                            return Some(CellStyleOverride {
                                bg_color: Some(vr.bg_color.clone()),
                                color: None, font: None,
                            });
                        }
                    }
                }
            }
        }
    }
    None
}
