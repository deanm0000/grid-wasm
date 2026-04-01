use crate::canvas::CanvasCtx;
use crate::theme::Theme;
use crate::types::{ContentAlign, GridCell, GridSelection, Rectangle};

/// Draw a rounded rectangle path (does not fill/stroke).
pub fn rounded_rect(ctx: &CanvasCtx, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    ctx.begin_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    // Top-right corner
    let _ = ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.line_to(x + w, y + h - r);
    let _ = ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.line_to(x + r, y + h);
    let _ = ctx.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    ctx.line_to(x, y + r);
    let _ = ctx.arc(x + r, y + r, r, std::f64::consts::PI, std::f64::consts::PI * 1.5);
    ctx.close_path();
}

/// Measure the vertical offset needed to center text vertically in a cell.
/// Returns a bias value to add to `y + h/2`.
pub fn get_middle_center_bias(ctx: &mut CanvasCtx, font: &str) -> f64 {
    let sample = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let m1 = ctx.measure_text(sample, font);

    // Approximate: actualBoundingBoxAscent is not always available from web-sys
    // We use a heuristic: bias ≈ -1 for typical 13px fonts
    let width = m1.width();
    if width > 0.0 {
        // Rough approximation based on font metrics
        // For a 13px font, typical ascent is ~10-11px
        let font_size = parse_font_size(font);
        -(font_size * 0.35)
    } else {
        0.0
    }
}

fn parse_font_size(font: &str) -> f64 {
    // font string is like "13px Inter, sans-serif" or "600 13px Inter"
    for part in font.split_whitespace() {
        if let Some(px) = part.strip_suffix("px") {
            if let Ok(size) = px.parse::<f64>() {
                return size;
            }
        }
    }
    13.0 // default
}

/// Draw a single line of text, aligned within a cell.
pub fn draw_single_text_line(
    ctx: &mut CanvasCtx,
    text: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    bias: f64,
    theme: &Theme,
    align: Option<ContentAlign>,
) {
    let center_y = y + h / 2.0 + bias;

    match align {
        Some(ContentAlign::Right) => {
            ctx.set_text_align("right");
            let _ = ctx.fill_text(text, x + w - (theme.cell_horizontal_padding + 0.5), center_y);
        }
        Some(ContentAlign::Center) => {
            ctx.set_text_align("center");
            let _ = ctx.fill_text(text, x + w / 2.0, center_y);
        }
        _ => {
            ctx.set_text_align("left");
            let _ = ctx.fill_text(text, x + theme.cell_horizontal_padding + 0.5, center_y);
        }
    }
}

/// Truncate a string so it fits within the given width.
pub fn truncate_string(ctx: &mut CanvasCtx, data: &str, w: f64, font: &str) -> String {
    // Take first line only (no wrapping)
    let first_line = data.lines().next().unwrap_or("");

    // Quick length heuristic
    let max_chars = (w / 4.0) as usize;
    if first_line.len() > max_chars {
        first_line.chars().take(max_chars).collect()
    } else {
        first_line.to_string()
    }
}

/// Draw text content in a cell with optional alignment and wrapping.
pub fn draw_text_cell(
    ctx: &mut CanvasCtx,
    data: &str,
    rect: &Rectangle,
    theme: &Theme,
    align: Option<ContentAlign>,
    font: &str,
) {
    if data.is_empty() {
        return;
    }

    let truncated = truncate_string(ctx, data, rect.width, font);
    let bias = get_middle_center_bias(ctx, font);

    // Detect RTL (simple heuristic)
    let is_rtl = is_rtl_text(&truncated);
    let align = if align.is_none() && is_rtl {
        Some(ContentAlign::Right)
    } else {
        align
    };

    if is_rtl {
        ctx.set_direction("rtl");
    }

    if matches!(align, Some(ContentAlign::Right)) {
        ctx.set_text_align("right");
    } else if matches!(align, Some(ContentAlign::Center)) {
        ctx.set_text_align("center");
    } else {
        ctx.set_text_align("left");
    }

    draw_single_text_line(ctx, &truncated, rect.x, rect.y, rect.width, rect.height, bias, theme, align);

    // Reset
    if is_rtl {
        ctx.set_direction("ltr");
        ctx.set_text_align("left");
    }
}

fn is_rtl_text(s: &str) -> bool {
    for c in s.chars() {
        let cp = c as u32;
        // Arabic
        if (0x0600..=0x06FF).contains(&cp) { return true; }
        // Hebrew
        if (0x0590..=0x05FF).contains(&cp) { return true; }
        // Persian
        if (0xFB50..=0xFDFF).contains(&cp) { return true; }
        // Arabic Supplement
        if (0x0750..=0x077F).contains(&cp) { return true; }
        // If we see a Latin letter, it's likely LTR
        if c.is_ascii_alphabetic() { return false; }
    }
    false
}

/// Check if a cell is selected in the current selection.
pub fn cell_is_selected(col: usize, row: i32, selection: &GridSelection) -> bool {
    if let Some(ref current) = selection.current {
        if current.cell.col == col as i32 && current.cell.row == row {
            return true;
        }
        // Check range
        let r = &current.range;
        (col as f64) >= r.x
            && (col as f64) < r.x + r.width
            && (row as f64) >= r.y
            && (row as f64) < r.y + r.height
    } else {
        false
    }
}
