use crate::columns::ResolvedColumns;
use crate::types::Rectangle;
use crate::walk::MappedColumn;

// ── Drawn element sizes (from right edge of column, leftward) ─────────────────

/// Gap between column right edge and the right side of the sort triangle.
const TRI_PADDING: f64 = 6.0;
/// Width/height of each sort triangle bounding box.
const TRI_SIZE: f64 = 7.0;
/// Vertical gap between the up and down triangles (center-to-center minus size).
const TRI_GAP: f64 = 3.0;
/// Gap between menu button center right edge and the triangle left edge (drawn).
const MENU_BTN_GAP: f64 = 4.0;
/// Extra hit padding above the up-triangle and below the down-triangle.
const TRI_HIT_PAD: f64 = 3.0;
/// Width of the ⋮ menu button bounding box.
pub const MENU_BTN_WIDTH: f64 = 14.0;

// ── Hit-area boundaries (all relative to right_border_x) ─────────────────────
//
// Layout from right to left (all values are offsets FROM right_border_x):
//
//   +3  resize right boundary
//    0  right_border_x (drawn column border)
//   -3  resize left boundary   = TRI_PADDING / 2
//   -6  tri right draw edge    = right_border_x - TRI_PADDING
//  -13  tri left draw edge     = right_border_x - TRI_PADDING - TRI_SIZE
//  -15  tri/menu boundary      = midpoint(-13, -17)
//  -17  menu right draw edge   = tri_cx - MENU_BTN_GAP - (MENU_BTN_WIDTH/2 - MENU_BTN_WIDTH)
//  -24  menu_btn_cx            = tri_cx - MENU_BTN_GAP - MENU_BTN_WIDTH/2
//  -31  menu left draw edge    = menu_btn_cx - MENU_BTN_WIDTH/2
//
// Non-overlapping hit zones (derived from draw positions, see compute_column_layout):
//   Resize:    ±RESIZE_HALF around right_border_x
//   Triangle:  from menu/tri boundary to resize left
//   Menu btn:  from draw left to menu/tri boundary

/// Half-width of the resize hit zone on each side of the border.
/// = TRI_PADDING / 2 so it touches but doesn't overlap the triangle.
pub const RESIZE_HALF: f64 = TRI_PADDING / 2.0;    // = 3.0


// ── Visual dot constants ──────────────────────────────────────────────────────
pub const MENU_DOT_RADIUS: f64 = 1.5;
pub const MENU_DOT_GAP: f64 = 4.0;

/// Minimum span pixel width before a ⋮ menu button is drawn on a parent header span.
const MIN_SPAN_WIDTH_FOR_MENU: f64 = 20.0;

#[derive(Clone, Debug)]
pub struct ColumnLayoutEntry {
    pub source_index: usize,
    pub draw_x: f64,
    pub width: f64,
    pub is_resizable: bool,
    pub tri_up_cx: f64,
    pub tri_up_cy: f64,
    pub tri_down_cx: f64,
    pub tri_down_cy: f64,
    pub tri_up_rect: Rectangle,
    pub tri_down_rect: Rectangle,
    pub right_border_x: f64,
    pub menu_btn_cx: f64,
    pub menu_btn_cy: f64,
    pub menu_btn_rect: Rectangle,
}

/// A clickable ⋮ button on a parent header span (non-leaf level).
#[derive(Clone, Debug)]
pub struct SpanMenuEntry {
    pub level_idx: usize,
    pub first_leaf: usize,
    pub last_leaf: usize,
    pub span_title: String,
    /// Pixel y of the span row (top edge).
    pub span_y: f64,
    /// Height of the span row.
    pub span_h: f64,
    pub menu_btn_cx: f64,
    pub menu_btn_cy: f64,
    pub menu_btn_rect: Rectangle,
}

#[derive(Clone, Debug)]
pub struct ColumnLayout {
    pub entries: Vec<ColumnLayoutEntry>,
    pub span_menus: Vec<SpanMenuEntry>,
    pub leaf_y: f64,
    pub leaf_h: f64,
    pub total_header_height: f64,
}

impl ColumnLayout {
    pub fn entry_by_source(&self, source_index: usize) -> Option<&ColumnLayoutEntry> {
        self.entries.iter().find(|e| e.source_index == source_index)
    }
}

pub fn compute_column_layout(
    effective_cols: &[MappedColumn],
    translate_x: f64,
    header_height: f64,
    group_header_height: f64,
    resolved: Option<&ResolvedColumns>,
    // Arrow names of aggregated (value) columns. Span menus only shown when Some.
    grouped_value_col_names: Option<&[String]>,
) -> ColumnLayout {
    let (leaf_y, leaf_h) = compute_leaf_row_geometry(
        header_height, group_header_height, resolved,
    );

    let tri_center_y = leaf_y + leaf_h / 2.0;
    let tri_up_cy = tri_center_y - TRI_SIZE / 2.0 - TRI_GAP / 2.0;
    let tri_down_cy = tri_center_y + TRI_SIZE / 2.0 + TRI_GAP / 2.0;

    let mut entries = Vec::with_capacity(effective_cols.len());
    let mut x_acc = 0.0f64;
    let mut clip_x = 0.0f64;

    for c in effective_cols {
        let raw_draw_x = if c.sticky { clip_x } else { x_acc + translate_x };
        let effective_clip_x = if c.sticky { 0.0 } else { clip_x };
        let diff = if effective_clip_x > raw_draw_x {
            effective_clip_x - raw_draw_x
        } else {
            0.0
        };
        let draw_x = raw_draw_x + diff;
        let w = c.width - diff;
        let right_border_x = draw_x + w;

        // ── Triangle positions ────────────────────────────────────────────────
        // tri_cx is the LEFT edge of the drawn triangle bounding box.
        let tri_cx = right_border_x - TRI_PADDING - TRI_SIZE;

        // ── Sort triangle hit zone ────────────────────────────────────────────
        // Drawn triangle center x: tri_cx + TRI_SIZE/2
        // Drawn triangle spans x: [tri_cx, tri_cx + TRI_SIZE]
        let tri_draw_center_x = tri_cx + TRI_SIZE / 2.0;

        // Right boundary: resize left (right_border_x - RESIZE_HALF). Non-negotiable.
        let tri_hit_x_right = right_border_x - RESIZE_HALF;

        // Half-width = distance from draw center to right boundary (symmetric).
        let tri_hit_half_x = tri_hit_x_right - tri_draw_center_x;

        // Left boundary = draw center - same half-width (symmetric hit zone).
        let tri_hit_x_left = tri_draw_center_x - tri_hit_half_x;

        let tri_hit_w = tri_hit_x_right - tri_hit_x_left;

        // Y bounds: centered on each drawn triangle with TRI_HIT_PAD.
        // Gap midpoint used as the y split to keep zones adjacent.
        let up_bottom = tri_up_cy + TRI_SIZE;
        let down_top = tri_down_cy;
        let y_split = (up_bottom + down_top) / 2.0;

        let tri_up_top    = tri_up_cy - TRI_HIT_PAD;
        let tri_up_bottom = y_split;
        let tri_down_top  = y_split;
        let tri_down_bottom = tri_down_cy + TRI_SIZE + TRI_HIT_PAD;

        let tri_up_rect   = Rectangle::new(tri_hit_x_left, tri_up_top,   tri_hit_w, tri_up_bottom   - tri_up_top);
        let tri_down_rect = Rectangle::new(tri_hit_x_left, tri_down_top, tri_hit_w, tri_down_bottom - tri_down_top);

        // ── Menu button positions ─────────────────────────────────────────────
        let menu_btn_cx = tri_cx - MENU_BTN_GAP - MENU_BTN_WIDTH / 2.0;
        let menu_btn_cy = leaf_y + leaf_h / 2.0;

        // Right boundary = tri hit left (adjacent, non-overlapping).
        let menu_hit_right = tri_hit_x_left;

        // Half-width = distance from draw center to right boundary (symmetric).
        let menu_hit_half_x = menu_hit_right - menu_btn_cx;

        // Left boundary = draw center - same half-width, clamped to column left edge.
        let menu_hit_left = (menu_btn_cx - menu_hit_half_x).max(draw_x);

        let menu_hit_w = menu_hit_right - menu_hit_left;
        let menu_btn_rect = Rectangle::new(
            menu_hit_left,
            leaf_y,
            menu_hit_w.max(0.0),
            leaf_h,
        );

        

        entries.push(ColumnLayoutEntry {
            source_index: c.source_index,
            draw_x,
            width: w,
            is_resizable: c.is_resizable,
            tri_up_cx: tri_cx,
            tri_up_cy,
            tri_down_cx: tri_cx,
            tri_down_cy,
            tri_up_rect,
            tri_down_rect,
            right_border_x,
            menu_btn_cx,
            menu_btn_cy,
            menu_btn_rect,
        });

        x_acc += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }

    // Compute span menu buttons for parent header levels (grouped mode only,
    // and only for value/aggregation columns — not group keys).
    let span_menus = if let Some(value_col_names) = grouped_value_col_names {
        compute_span_menus(
            effective_cols, translate_x,
            header_height, group_header_height,
            resolved, value_col_names,
        )
    } else {
        Vec::new()
    };

    ColumnLayout {
        entries,
        span_menus,
        leaf_y,
        leaf_h,
        total_header_height: header_height + group_header_height,
    }
}

/// Compute ⋮ button positions for parent spans that correspond to value (aggregated) columns.
fn compute_span_menus(
    effective_cols: &[MappedColumn],
    translate_x: f64,
    header_height: f64,
    group_header_height: f64,
    resolved: Option<&ResolvedColumns>,
    value_col_names: &[String],
) -> Vec<SpanMenuEntry> {
    let resolved = match resolved {
        Some(r) if r.max_depth > 1 => r,
        _ => return Vec::new(),
    };

    let total = header_height + group_header_height;
    let level_h = total / resolved.max_depth as f64;

    let mut result = Vec::new();

    for (level_idx, spans) in resolved.header_levels.iter().enumerate() {
        let span_y = level_idx as f64 * level_h;
        let span_h = level_h;
        let span_cy = span_y + span_h / 2.0;

        for span in spans {
            if span.title.is_empty() {
                continue;
            }

            // Only draw ⋮ on value (aggregation) spans, not on group key spans.
            // A span is a value span if it covers leaves whose parent title
            // matches a value column name. We identify by checking if the span
            // title is "Group" (which covers group keys) or if none of the
            // leaves in the span belong to value columns.
            let is_value_span = span_corresponds_to_value_col(
                span.first_leaf, span.last_leaf, &span.title, resolved, value_col_names,
            );
            if !is_value_span {
                continue;
            }

            let (span_x, span_w) = compute_span_pixel_bounds(
                span.first_leaf, span.last_leaf, effective_cols, translate_x,
            );

            if span_w <= MIN_SPAN_WIDTH_FOR_MENU {
                continue;
            }

            // Use the same right-edge geometry as leaf columns for the ⋮ button.
            let span_right = span_x + span_w;
            let btn_cx = span_right - TRI_PADDING - MENU_BTN_WIDTH / 2.0;
            let btn_cy = span_cy;
            let btn_draw_left = btn_cx - MENU_BTN_WIDTH / 2.0;
            let btn_hit_right = span_right - TRI_PADDING / 2.0;
            let btn_rect = Rectangle::new(
                btn_draw_left,
                span_y,
                (btn_hit_right - btn_draw_left).max(0.0),
                span_h,
            );

            result.push(SpanMenuEntry {
                level_idx,
                first_leaf: span.first_leaf,
                last_leaf: span.last_leaf,
                span_title: span.title.clone(),
                span_y,
                span_h,
                menu_btn_cx: btn_cx,
                menu_btn_cy: btn_cy,
                menu_btn_rect: btn_rect,
            });
        }
    }

    result
}

/// Determine if a header span corresponds to a value (aggregation) column.
/// Group-key spans are titled "Group" and their leaves' parent_titles[level_idx] = "Group".
/// Value spans are titled with the original column display name.
fn span_corresponds_to_value_col(
    first_leaf: usize,
    last_leaf: usize,
    span_title: &str,
    resolved: &ResolvedColumns,
    value_col_names: &[String],
) -> bool {
    // "Group" span always covers group keys
    if span_title == "Group" {
        return false;
    }
    // Check if any leaf in this span belongs to a value column
    resolved.leaves.iter()
        .filter(|l| l.display_index >= first_leaf && l.display_index <= last_leaf)
        .any(|l| value_col_names.iter().any(|v| v == &l.arrow_name || {
            // The leaf's arrow_name may be "cost_sum" — check if its parent title matches
            l.parent_titles.last().map_or(false, |p| p == span_title)
        }))
}

fn compute_span_pixel_bounds(
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

pub fn compute_leaf_row_geometry(
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
    let y = group_header_height;
    let h = header_height - group_header_height;
    (y, h)
}

pub fn hit_test_sort_triangle(
    px: f64,
    py: f64,
    layout: &ColumnLayout,
) -> Option<(usize, bool)> {
    if py < layout.leaf_y || py > layout.leaf_y + layout.leaf_h {
        return None;
    }
    for entry in &layout.entries {
        if entry.tri_up_rect.contains(px, py) {
            
            return Some((entry.source_index, true));
        }
        if entry.tri_down_rect.contains(px, py) {
            
            return Some((entry.source_index, false));
        }
    }
    None
}

pub fn hit_test_resize_border(
    px: f64,
    py: f64,
    layout: &ColumnLayout,
) -> Option<usize> {
    if py < 0.0 || py > layout.total_header_height {
        return None;
    }
    for entry in &layout.entries {
        if entry.is_resizable
            && px >= entry.right_border_x - RESIZE_HALF
            && px <= entry.right_border_x + RESIZE_HALF
        {
            
            return Some(entry.source_index);
        }
    }
    None
}

pub fn hit_test_menu_button(
    px: f64,
    py: f64,
    layout: &ColumnLayout,
) -> Option<usize> {
    if py < layout.leaf_y || py > layout.leaf_y + layout.leaf_h {
        return None;
    }
    for entry in &layout.entries {
        if entry.menu_btn_rect.contains(px, py) {
            
            return Some(entry.source_index);
        }
    }
    None
}

/// Returns the `SpanMenuEntry` hit, if any. Must be in a non-leaf header row.
pub fn hit_test_span_menu_button(
    px: f64,
    py: f64,
    layout: &ColumnLayout,
) -> Option<&SpanMenuEntry> {
    if py >= layout.leaf_y && py <= layout.leaf_y + layout.leaf_h {
        return None;
    }
    for span in &layout.span_menus {
        if span.menu_btn_rect.contains(px, py) {
            return Some(span);
        }
    }
    None
}

pub fn tri_size() -> f64 {
    TRI_SIZE
}

/// Total pixel width reserved on the right side of each leaf header cell for controls.
pub fn header_right_reserved_width() -> f64 {
    // From draw_x + w, we reserve: resize gap + triangle + gap + menu button
    TRI_PADDING + TRI_SIZE + MENU_BTN_GAP + MENU_BTN_WIDTH
}
