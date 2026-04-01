use crate::walk::MappedColumn;

/// Returns the source column index for a given x position, or -1 if not found.
pub fn get_column_index_for_x(
    target_x: f64,
    effective_columns: &[MappedColumn],
    translate_x: f64,
) -> i32 {
    let mut x = 0.0f64;
    for c in effective_columns {
        let cx = if c.sticky { x } else { x + translate_x };
        if target_x <= cx + c.width {
            return c.source_index as i32;
        }
        x += c.width;
    }
    -1
}

/// Returns the row index for a given y position.
/// Returns:
///   -2 for group header
///   -1 for column header
///   0+ for data rows
///   None if beyond the grid
pub fn get_row_index_for_y(
    target_y: f64,
    height: f64,
    has_groups: bool,
    header_height: f64,
    group_header_height: f64,
    rows: usize,
    row_height: f64,
    cell_y_offset: usize,
    translate_y: f64,
    freeze_trailing_rows: usize,
) -> Option<i32> {
    let total_header_height = header_height + group_header_height;

    if has_groups && target_y <= group_header_height {
        return Some(-2);
    }
    if target_y <= total_header_height {
        return Some(-1);
    }

    // Check freeze trailing rows
    let mut y = height;
    for fr in 0..freeze_trailing_rows {
        let row = rows - 1 - fr;
        y -= row_height;
        if target_y >= y {
            return Some(row as i32);
        }
    }

    let effective_rows = rows - freeze_trailing_rows;
    let ty = target_y - translate_y;
    let target = ((ty - total_header_height) / row_height).floor() as usize + cell_y_offset;

    if target >= effective_rows {
        None
    } else {
        Some(target as i32)
    }
}

/// Returns the column index at pixel position (px, py), accounting for translate offsets.
/// Returns None if outside the visible grid.
pub fn hit_test(
    px: f64,
    py: f64,
    effective_columns: &[MappedColumn],
    width: f64,
    height: f64,
    has_groups: bool,
    header_height: f64,
    group_header_height: f64,
    rows: usize,
    row_height: f64,
    cell_x_offset: usize,
    cell_y_offset: usize,
    translate_x: f64,
    translate_y: f64,
    freeze_trailing_rows: usize,
) -> Option<(i32, i32)> {
    if px < 0.0 || px >= width || py < 0.0 || py >= height {
        return None;
    }

    let col = get_column_index_for_x(px, effective_columns, translate_x);
    let row = get_row_index_for_y(
        py,
        height,
        has_groups,
        header_height,
        group_header_height,
        rows,
        row_height,
        cell_y_offset,
        translate_y,
        freeze_trailing_rows,
    )?;

    if col < 0 {
        return None;
    }

    Some((col, row))
}

/// Computes the pixel bounds of a cell at (col, row).
pub fn get_cell_bounds(
    col: usize,
    row: i32,
    effective_columns: &[MappedColumn],
    height: f64,
    header_height: f64,
    group_header_height: f64,
    rows: usize,
    row_height: f64,
    cell_y_offset: usize,
    translate_x: f64,
    translate_y: f64,
    freeze_trailing_rows: usize,
) -> Option<(f64, f64, f64, f64)> {
    let total_header_height = header_height + group_header_height;

    // Find x position
    let mut x: f64 = 0.0;
    let mut clip_x: f64 = 0.0;
    let mut found_x = None;
    let mut col_width = 0.0;

    for c in effective_columns {
        if c.source_index == col {
            let draw_x = if c.sticky { clip_x } else { x + translate_x };
            found_x = Some(draw_x);
            col_width = c.width;
            break;
        }
        x += c.width;
        if c.sticky {
            clip_x += c.width;
        }
    }

    let draw_x = found_x?;

    // Find y position
    let draw_y = if row == -2 {
        0.0
    } else if row == -1 {
        group_header_height
    } else {
        let row_idx = row as usize;
        if row_idx >= rows {
            return None;
        }

        // Check freeze trailing
        if row_idx >= rows - freeze_trailing_rows {
            let mut y = height;
            for fr in 0..freeze_trailing_rows {
                let r = rows - 1 - fr;
                y -= row_height;
                if r == row_idx {
                    return Some((draw_x, y, col_width, row_height));
                }
            }
            return None;
        }

        let y = total_header_height + translate_y
            + ((row_idx as isize - cell_y_offset as isize) as f64) * row_height;
        y
    };

    let h = if row == -2 {
        group_header_height
    } else if row == -1 {
        header_height - group_header_height
    } else {
        row_height
    };

    Some((draw_x, draw_y, col_width, h))
}
