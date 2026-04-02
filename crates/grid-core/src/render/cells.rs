use crate::canvas::CanvasCtx;
use crate::color::blend;
use crate::columns::ResolvedColumns;
use crate::number_format::{format_accounting_parts, format_number, is_accounting};
use crate::theme::Theme;
use crate::types::{
    CellStyleOverride, ConditionalRule, ContentAlign, DataStyle, GridCell, GridSelection, Rectangle,
};
use crate::walk::{walk_columns, walk_rows_in_col, MappedColumn};

use super::lib_utils::{cell_is_selected, draw_text_cell, get_middle_center_bias, rounded_rect};

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
    resolved: Option<&ResolvedColumns>,
) {
    let font = theme.base_font_full();

    walk_columns(
        effective_cols,
        cell_y_offset,
        translate_x,
        translate_y,
        total_header_height,
        |c, draw_x, _col_draw_y, clip_x, start_row| {
            let diff = if clip_x > draw_x { clip_x - draw_x } else { 0.0 };
            let col_draw_x = draw_x + diff;
            let col_draw_y = total_header_height + 1.0;
            let col_width = c.width - diff;
            let col_height = height - total_header_height - 1.0;

            if col_draw_x > height || col_width <= 0.0 {
                return false;
            }

            ctx.save();
            ctx.clip_rect(col_draw_x, col_draw_y, col_width, col_height);

            let leaf = resolved.and_then(|r| r.leaf_by_display_index(c.source_index));
            let data_style = leaf.and_then(|l| l.data_style.as_ref());

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
                    let cell_x = col_draw_x;
                    let cell_y = draw_y;
                    let cell_w = col_width;
                    let cell_h = rh;

                    let cell = get_cell_content(c.source_index as i32, row as i32);

                    let cond_style = data_style
                        .and_then(|ds| ds.conditional_formats.as_ref())
                        .and_then(|rules| evaluate_conditions(rules, &cell));

                    draw_cell_background(
                        ctx, cell_x, cell_y, cell_w, cell_h,
                        c.source_index, row as i32, selection,
                        theme, data_style, cond_style.as_ref(),
                    );

                    draw_cell_content(
                        ctx, &cell, cell_x, cell_y, cell_w, cell_h,
                        theme, &font, data_style, cond_style.as_ref(),
                    );

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
    ctx.fill_rect(x + 1.0, y, w - 1.0, h);
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
            let display = display_data.as_deref().unwrap_or(data);
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

    // Symbol: left-pinned at padding
    ctx.set_text_align("left");
    let _ = ctx.fill_text(symbol, rect.x + pad, center_y);

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
    let sw = skeleton_width.unwrap_or(w * 0.6).min(w - padding * 2.0);
    let sh = 8.0;
    let sx = x + padding;
    let sy = y + (h - sh) / 2.0;
    ctx.set_fill_style(&theme.border_color);
    rounded_rect(ctx, sx, sy, sw, sh, 4.0);
    ctx.fill();
}

fn evaluate_conditions(
    rules: &[ConditionalRule],
    cell: &GridCell,
) -> Option<CellStyleOverride> {
    let numeric_val = match cell {
        GridCell::Number { data: Some(v), .. } => Some(*v),
        _ => None,
    };
    let string_val = match cell {
        GridCell::Text { data, .. } => Some(data.as_str()),
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
                if is_null {
                    return Some(style.clone());
                }
            }
            ConditionalRule::IsNotNull { style } => {
                if !is_null {
                    return Some(style.clone());
                }
            }
            ConditionalRule::Percentile { .. } => {
                // Percentile requires pre-computation of column stats
                // which is done at a higher level — skip here
            }
        }
    }
    None
}
