use crate::types::GridColumn;

#[derive(Clone)]
pub struct MappedColumn {
    pub title: String,
    pub width: f64,
    pub group: Option<String>,
    pub icon: Option<String>,
    pub source_index: usize,
    pub sticky: bool,
}

impl MappedColumn {
    pub fn from_column(col: &GridColumn, source_index: usize, sticky: bool) -> Self {
        Self {
            title: col.title.clone(),
            width: col.width,
            group: col.group.clone(),
            icon: col.icon.clone(),
            source_index,
            sticky,
        }
    }
}

/// Walk visible columns, calling `cb` for each column that should be drawn.
/// Yields: (column, draw_x, col_draw_y, clip_x, start_row)
pub fn walk_columns<F>(
    effective_cols: &[MappedColumn],
    cell_y_offset: usize,
    translate_x: f64,
    translate_y: f64,
    total_header_height: f64,
    mut cb: F,
) where
    F: FnMut(&MappedColumn, f64, f64, f64, usize) -> bool,
{
    let mut x: f64 = 0.0;
    let mut clip_x: f64 = 0.0;
    let draw_y = total_header_height + translate_y;

    for c in effective_cols {
        let draw_x = if c.sticky { clip_x } else { x + translate_x };
        let effective_clip_x = if c.sticky { 0.0 } else { clip_x };

        if cb(c, draw_x, draw_y, effective_clip_x, cell_y_offset) {
            break;
        }

        x += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }
}

pub type WalkRowsCallback = dyn FnMut(f64, usize, f64, bool, bool) -> bool;

/// Walk visible rows in a column, calling `cb` for each row.
/// Yields: (draw_y, row, row_height, is_sticky, is_trailing_row)
pub fn walk_rows_in_col(
    start_row: usize,
    draw_y: f64,
    height: f64,
    rows: usize,
    get_row_height: impl Fn(usize) -> f64,
    freeze_trailing_rows: usize,
    has_append_row: bool,
    skip_to_y: Option<f64>,
    mut cb: impl FnMut(f64, usize, f64, bool, bool) -> bool,
) {
    let skip_to_y = skip_to_y.unwrap_or(draw_y);
    let mut y = draw_y;
    let mut row = start_row;
    let row_end = rows.saturating_sub(freeze_trailing_rows);
    let mut did_break = false;

    while y < height && row < row_end {
        let rh = get_row_height(row);
        if y + rh > skip_to_y {
            let is_trailing = has_append_row && row == rows - 1;
            if cb(y, row, rh, false, is_trailing) {
                did_break = true;
                break;
            }
        }
        y += rh;
        row += 1;
    }

    if did_break {
        return;
    }

    // Freeze trailing rows - drawn from bottom up
    let mut y = height;
    for fr in 0..freeze_trailing_rows {
        let row = rows - 1 - fr;
        let rh = get_row_height(row);
        y -= rh;
        let is_trailing = has_append_row && row == rows - 1;
        cb(y, row, rh, true, is_trailing);
    }
}

pub struct WalkGroupResult {
    pub col_start: usize,
    pub col_end: usize,
    pub group: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Walk group headers, yielding spans of columns that share a group.
pub fn walk_groups(
    effective_cols: &[MappedColumn],
    width: f64,
    translate_x: f64,
    group_header_height: f64,
    mut cb: impl FnMut(&WalkGroupResult),
) {
    let mut x: f64 = 0.0;
    let mut clip_x: f64 = 0.0;
    let mut group_start = 0usize;
    let mut current_group: Option<String> = None;

    for (i, col) in effective_cols.iter().enumerate() {
        let draw_x = if col.sticky { clip_x } else { x + translate_x };

        let group_name = col.group.clone().unwrap_or_default();
        let is_new_group = current_group.as_ref().map_or(true, |g| *g != group_name);

        if is_new_group && i > 0 {
            // Emit the previous group
            let prev_col = &effective_cols[i - 1];
            let group_x = if effective_cols[group_start].sticky {
                // Calculate from start
                let mut gx = 0.0;
                for c in &effective_cols[..group_start] {
                    if c.sticky {
                        gx += c.width;
                    }
                }
                gx
            } else {
                let mut gx = 0.0;
                for c in &effective_cols[..group_start] {
                    gx += c.width;
                }
                gx + translate_x
            };
            let mut gw = 0.0;
            for c in &effective_cols[group_start..i] {
                gw += c.width;
            }

            cb(&WalkGroupResult {
                col_start: group_start,
                col_end: i - 1,
                group: current_group.clone().unwrap_or_default(),
                x: group_x,
                y: 0.0,
                width: gw,
                height: group_header_height,
            });
            group_start = i;
        }

        current_group = Some(group_name);
        x += col.width;
        if col.sticky {
            clip_x += col.width;
        }
    }

    // Emit final group
    if let Some(group) = current_group {
        let group_x = if effective_cols[group_start].sticky {
            let mut gx = 0.0;
            for c in &effective_cols[..group_start] {
                if c.sticky {
                    gx += c.width;
                }
            }
            gx
        } else {
            let mut gx = 0.0;
            for c in &effective_cols[..group_start] {
                gx += c.width;
            }
            gx + translate_x
        };
        let mut gw = 0.0;
        for c in &effective_cols[group_start..] {
            gw += c.width;
        }
        cb(&WalkGroupResult {
            col_start: group_start,
            col_end: effective_cols.len() - 1,
            group,
            x: group_x,
            y: 0.0,
            width: gw,
            height: group_header_height,
        });
    }
}
