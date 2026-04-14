pub mod arrow_data;
pub mod canvas;
pub mod color;
pub mod columns;
pub mod defaults;
pub mod grid;
pub mod hit_test;
pub mod layout;
pub mod number_format;
pub mod render;
pub mod theme;
pub mod types;
pub mod walk;

use defaults::*;

use canvas::CanvasCtx;
use grid::GridState;
use types::{AggregateFunction, ColDragState, ColSlideAnimation, ColumnInput, DateTruncation, ResizeState, SortDirection, SortState};
use wasm_bindgen::prelude::*;

/// Each entry is (strftime_token, separator_that_follows_it_in_the_full_string).
/// The chain is ordered coarsest → finest. The full format string is produced by
/// concatenating token+separator for every entry (last entry has separator "").
fn truncation_format_chain(t: DateTruncation) -> &'static [(&'static str, &'static str)] {
    match t {
        DateTruncation::Year       => &[("%Y", "")],
        DateTruncation::Quarter    => &[("%Y", "-"), ("%m", "")],
        DateTruncation::Month      => &[("%Y", "-"), ("%m", "")],
        DateTruncation::Week       => &[("%Y", "-W"), ("%W", "")],
        DateTruncation::Day        => &[("%Y", "-"), ("%m", "-"), ("%d", "")],
        DateTruncation::Hour       => &[("%Y", "-"), ("%m", "-"), ("%d", " "), ("%H", "")],
        DateTruncation::Minute     => &[("%Y", "-"), ("%m", "-"), ("%d", " "), ("%H", ":"), ("%M", "")],
        DateTruncation::Second     => &[("%Y", "-"), ("%m", "-"), ("%d", " "), ("%H", ":"), ("%M", ":"), ("%S", "")],
        DateTruncation::Millisecond => &[("%Y", "-"), ("%m", "-"), ("%d", " "), ("%H", ":"), ("%M", ":"), ("%S", "."), ("%3f", "")],
        DateTruncation::Microsecond => &[("%Y", "-"), ("%m", "-"), ("%d", " "), ("%H", ":"), ("%M", ":"), ("%S", "."), ("%6f", "")],
        DateTruncation::Nanosecond  => &[("%Y", "-"), ("%m", "-"), ("%d", " "), ("%H", ":"), ("%M", ":"), ("%S", "."), ("%9f", "")],
    }
}

/// The single strftime token that represents "this truncation level's own component"
/// (i.e. the last / finest element in its chain).
fn truncation_own_token(t: DateTruncation) -> &'static str {
    truncation_format_chain(t).last().map(|(tok, _)| *tok).unwrap_or("")
}

/// Compute the default NumberFormat for a newly-added truncated group key.
/// Looks at sibling keys for the same source column that already exist in group_keys,
/// strips the components they cover from the front of the new key's format chain,
/// and joins the remainder into a strftime format string.
fn compute_default_truncation_format(
    arrow_name: &str,
    new_trunc: DateTruncation,
    group_keys: &[types::DateGroupKey],
) -> types::NumberFormat {
    let chain = truncation_format_chain(new_trunc);

    // Collect all tokens covered by existing sibling keys (same source, different truncation).
    // A sibling covers every token in its own full chain, not just its finest token.
    // e.g. a Month sibling covers both %Y and %m, so Day added alongside Month strips both.
    let covered_tokens: std::collections::HashSet<&str> = group_keys.iter()
        .filter(|k| k.arrow_name == arrow_name)
        .filter_map(|k| k.truncation)
        .filter(|&t| t != new_trunc)
        .flat_map(|t| truncation_format_chain(t).iter().map(|(tok, _)| *tok))
        .collect();

    // Walk the chain from the start; strip any token that is already covered by a sibling.
    // We remove from the front because the chain goes coarse→fine and siblings are coarser.
    let remaining: Vec<(&str, &str)> = chain.iter()
        .copied()
        .filter(|(tok, _)| !covered_tokens.contains(tok))
        .collect();

    let format_str = if remaining.is_empty() {
        chain.last().map(|(tok, _)| tok.to_string()).unwrap_or_default()
    } else {
        let mut s = String::new();
        for (i, (tok, sep)) in remaining.iter().enumerate() {
            s.push_str(tok);
            if i < remaining.len() - 1 {
                s.push_str(sep);
            }
        }
        s
    };

    types::NumberFormat::DateTime { format: format_str }
}
use web_sys::HtmlCanvasElement;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct DataGrid {
    /// The canvas rendering context. Lives here, not in GridState,
    /// because it is a rendering concern rather than a logical state concern.
    canvas: CanvasCtx,
    state: GridState,
}

impl DataGrid {
    /// Render the current state to the canvas. Called from every pub render method.
    fn do_render(&mut self) {
        self.state.render(&mut self.canvas);
    }
}

#[wasm_bindgen]
impl DataGrid {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<DataGrid, JsValue> {
        let ctx = canvas
            .get_context("2d")?
            .ok_or("Failed to get 2d context")?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        let mut state = GridState::new();
        state.width = width;
        state.height = height;

        Ok(DataGrid { canvas: CanvasCtx::new(ctx), state })
    }

    /// Set the grid dimensions (called from JS ResizeObserver on every canvas resize).
    pub fn set_size(&mut self, width: f64, height: f64) {
        self.state.width = width;
        self.state.height = height;
    }

    /// Set column definitions as a JSON array.
    /// Each column: { title: string, width: number, group?: string, icon?: string }
    pub fn set_columns(&mut self, cols: JsValue) -> Result<(), JsValue> {
        let columns: Vec<types::GridColumn> = serde_wasm_bindgen::from_value(cols)?;
        self.state.columns = columns;
        self.state.remap_columns();
        Ok(())
    }

    /// Set the number of data rows.
    pub fn set_rows(&mut self, rows: usize) {
        self.state.rows = rows;
    }

    /// Set a JS callback function that returns cell content.
    /// Signature: (col: number, row: number) => GridCell
    pub fn set_cell_callback(&mut self, cb: js_sys::Function) {
        self.state.get_cell_content_cb = Some(cb);
    }

    // --- Arrow-based data sources ---

    /// Set data from Arrow IPC stream bytes.
    pub fn set_data_ipc(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.state
            .set_data_from_ipc(bytes)
            .map_err(|e| JsValue::from_str(&e))?;
        self.state.auto_configure_from_data(DEFAULT_COLUMN_WIDTH);
        Ok(())
    }

    /// Set data from Arrow IPC stream bytes with custom column width.
    pub fn set_data_ipc_with_width(&mut self, bytes: &[u8], default_col_width: f64) -> Result<(), JsValue> {
        self.state
            .set_data_from_ipc(bytes)
            .map_err(|e| JsValue::from_str(&e))?;
        self.state.auto_configure_from_data(default_col_width);
        Ok(())
    }

    /// Set data from a JSON array of objects.
    pub fn set_data_objects(&mut self, json_str: &str) -> Result<(), JsValue> {
        let objects: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid JSON: {}", e)))?;

        let data = arrow_data::ArrowDataSource::from_json_objects(&objects)
            .map_err(|e| JsValue::from_str(&e))?;

        self.state.rows = data.num_rows();
        self.state.arrow_data = Some(data);
        self.state.auto_configure_from_data(DEFAULT_COLUMN_WIDTH);
        self.state.save_original_bytes();
        Ok(())
    }

    /// Set data from a JSON array of objects with custom default column width.
    pub fn set_data_objects_with_width(
        &mut self,
        json_str: &str,
        default_col_width: f64,
    ) -> Result<(), JsValue> {
        let objects: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid JSON: {}", e)))?;

        let data = arrow_data::ArrowDataSource::from_json_objects(&objects)
            .map_err(|e| JsValue::from_str(&e))?;

        self.state.rows = data.num_rows();
        self.state.arrow_data = Some(data);
        self.state.auto_configure_from_data(default_col_width);
        self.state.save_original_bytes();
        Ok(())
    }

    /// Initialize DataFusion for the current data source.
    pub fn init_datafusion(&mut self, table_name: &str) -> Result<(), JsValue> {
        if let Some(ref mut data) = self.state.arrow_data {
            data.init_datafusion(table_name)
                .map_err(|e| JsValue::from_str(&e))
        } else {
            Err(JsValue::from_str(
                "No data loaded. Call set_data_ipc or set_data_objects first.",
            ))
        }
    }

    /// Execute a SQL query against the registered data.
    pub async fn execute_query(&mut self, sql: &str) -> Result<(), JsValue> {
        let result = if let Some(ref data) = self.state.arrow_data {
            data.execute_query(sql).await
        } else {
            Err("No data loaded".to_string())
        };

        match result {
            Ok(new_data) => {
                self.state.rows = new_data.num_rows();
                self.state.arrow_data = Some(new_data);
                self.state.auto_configure_from_data(DEFAULT_COLUMN_WIDTH);
                Ok(())
            }
            Err(e) => Err(JsValue::from_str(&e)),
        }
    }

    // --- Column configuration ---

    pub fn set_column_input(&mut self, json_str: &str) -> Result<(), JsValue> {
        let inputs: Vec<ColumnInput> = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid column JSON: {}", e)))?;
        self.state.column_input = Some(inputs);
        self.state.column_overrides = None;
        self.state.auto_configure_from_data(DEFAULT_COLUMN_WIDTH);
        Ok(())
    }

    pub fn set_column_overrides(&mut self, json_str: &str) -> Result<(), JsValue> {
        let inputs: Vec<ColumnInput> = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid column overrides JSON: {}", e)))?;
        self.state.column_overrides = Some(inputs);
        self.state.column_input = None;
        self.state.auto_configure_from_data(DEFAULT_COLUMN_WIDTH);
        Ok(())
    }

    // --- Sorting ---

    /// Get the current sort state as JSON string: { column: number, direction: "asc"|"desc" } | null
    pub fn get_sort_state(&self) -> String {
        match &self.state.sort_state {
            SortState { column: Some(col), direction: Some(dir) } => {
                let dir_str = match dir {
                    SortDirection::Ascending => "asc",
                    SortDirection::Descending => "desc",
                };
                format!("{{\"column\":{},\"direction\":\"{}\"}}", col, dir_str)
            }
            _ => "null".to_string(),
        }
    }

    /// Set sort state. Column -1 clears sort. Direction: "asc" or "desc".
    pub fn set_sort_state(&mut self, column: i32, direction: &str) -> Result<(), JsValue> {
        if column < 0 {
            self.state.sort_state = SortState::default();
            // Restore original data only when not in grouped mode
            if !self.state.group_by_state.is_active() {
                self.restore_original_data()?;
            }
            return Ok(());
        }

        let dir = match direction {
            "asc" => SortDirection::Ascending,
            "desc" => SortDirection::Descending,
            _ => return Err(JsValue::from_str("direction must be 'asc' or 'desc'")),
        };

        self.state.sort_state = SortState {
            column: Some(column as usize),
            direction: Some(dir),
        };
        Ok(())
    }

    /// Apply the current sort state. This is async and returns a Promise.
    pub async fn apply_sort(&mut self) -> Result<(), JsValue> {
        // In grouped mode, sort applies to grouped_data (the aggregated result).
        // We leave arrow_data (and its original_ipc_bytes) untouched.
        if self.state.group_by_state.is_active() {
            let (col, ascending) = match &self.state.sort_state {
                SortState { column: Some(c), direction: Some(dir) } => {
                    (*c, matches!(dir, SortDirection::Ascending))
                }
                _ => return Ok(()), // no sort state → nothing to do in grouped mode
            };
            let arrow_col = self.state.display_to_arrow_index(col);
            if let Some(ref data) = self.state.grouped_data {
                let sorted = data.sort_by_column(arrow_col, ascending).await
                    .map_err(|e| JsValue::from_str(&e))?;
                self.state.rows = sorted.num_rows();
                self.state.grouped_data = Some(sorted);
            }
            return Ok(());
        }

        let (col, ascending) = match &self.state.sort_state {
            SortState { column: Some(c), direction: Some(dir) } => {
                (*c, matches!(dir, SortDirection::Ascending))
            }
            _ => {
                // Defensive: restore only when not in grouped mode
                if !self.state.group_by_state.is_active() {
                    self.restore_original_data()?;
                }
                return Ok(());
            }
        };

        // Translate display_index → arrow_index for the sort
        let arrow_col = self.state.display_to_arrow_index(col);

        if let Some(ref data) = self.state.arrow_data {
            let sorted = data.sort_by_column(arrow_col, ascending).await
                .map_err(|e| JsValue::from_str(&e))?;
            self.state.rows = sorted.num_rows();
            self.state.arrow_data = Some(sorted);
            self.state.remap_selection_after_sort();
            // Recompute date truncation caches now that data is properly sorted
            self.state.recompute_stale_date_truncations();
        }

        Ok(())
    }

    /// Clear sort and restore original data order.
    pub fn clear_sort(&mut self) -> Result<(), JsValue> {
        self.state.sort_state = SortState::default();
        if !self.state.group_by_state.is_active() {
            self.restore_original_data()?;
            self.state.remap_selection_after_sort();
        }
        Ok(())
    }

    // --- Data introspection ---

    /// Get the number of rows from the current data source.
    pub fn get_row_count(&self) -> usize {
        self.state.rows
    }

    /// Get the number of columns from the current data source.
    pub fn get_column_count(&self) -> usize {
        self.state.columns.len()
    }

    /// Get column schema info as JSON string: [{name, type}, ...]
    pub fn get_schema(&self) -> String {
        if let Some(ref data) = self.state.arrow_data {
            let schema: Vec<serde_json::Value> = (0..data.num_columns())
                .map(|i| {
                    serde_json::json!({
                        "name": data.column_name(i),
                        "type": format!("{}", data.column_type(i)),
                    })
                })
                .collect();
            serde_json::to_string(&schema).unwrap_or_else(|_| "[]".to_string())
        } else {
            let schema: Vec<serde_json::Value> = self.state.columns.iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.title,
                        "type": "unknown",
                    })
                })
                .collect();
            serde_json::to_string(&schema).unwrap_or_else(|_| "[]".to_string())
        }
    }

    /// Set header height in pixels.
    pub fn set_header_height(&mut self, h: f64) {
        self.state.header_height = h;
        // Recalculate group_header_height if we have multi-level columns
        if let Some(ref resolved) = self.state.resolved_columns {
            if resolved.max_depth > 1 {
                let level_height = h / resolved.max_depth as f64;
                self.state.group_header_height = level_height * (resolved.max_depth - 1) as f64;
            }
        }
    }

    /// Set row height in pixels.
    pub fn set_row_height(&mut self, h: f64) {
        self.state.row_height = h;
    }

    /// Set freeze columns count.
    pub fn set_freeze_columns(&mut self, n: usize) {
        self.state.freeze_columns = n;
        self.state.remap_columns();
    }

    /// Set freeze trailing rows count.
    pub fn set_freeze_trailing_rows(&mut self, n: usize) {
        self.state.freeze_trailing_rows = n;
    }

    pub fn set_swap_animation_duration(&mut self, ms: f64) {
        self.state.swap_animation_duration_ms = ms;
    }

    /// Set the theme as a JSON object matching the Theme struct.
    pub fn set_theme(&mut self, theme: JsValue) -> Result<(), JsValue> {
        let t: theme::Theme = serde_wasm_bindgen::from_value(theme)?;
        self.state.theme = t;
        Ok(())
    }

    /// Set the current scroll position (cell offsets).
    pub fn set_scroll(&mut self, cell_x_offset: usize, cell_y_offset: usize) {
        self.state.cell_x_offset = cell_x_offset;
        self.state.cell_y_offset = cell_y_offset;
    }

    /// Set sub-cell scroll translate (for smooth scrolling).
    pub fn set_translate(&mut self, x: f64, y: f64) {
        self.state.translate_x = x;
        self.state.translate_y = y;
    }

    /// Set the current selection as a JSON object.
    pub fn set_selection(&mut self, sel: JsValue) -> Result<(), JsValue> {
        if sel.is_null() || sel.is_undefined() {
            self.state.selection = types::GridSelection::default();
            return Ok(());
        }
        let s: types::GridSelection = serde_wasm_bindgen::from_value(sel)?;
        self.state.selection = s;
        Ok(())
    }

    /// Set focus state.
    pub fn set_focused(&mut self, focused: bool) {
        self.state.is_focused = focused;
    }

    /// Render the grid to the canvas.
    pub fn render(&mut self) {
        self.do_render();
    }

    // --- Event forwarding (called from JS) ---

    /// Handle click events.
    /// Plain cell selection is owned by on_drag_end (mouseup path).
    /// on_click handles: sort triangles, shift+click range extend, ctrl+click toggle.
    pub fn on_click(&mut self, x: f64, y: f64, shift: bool, ctrl: bool) {
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;

        // Sort triangle clicks — use the precomputed layout
        if y >= 0.0 && y <= hh + ghh {
            if let Some((col_idx, is_up)) = layout::hit_test_sort_triangle(
                x, y, &self.state.col_layout,
            ) {
                let was_active = self.state.sort_state.column == Some(col_idx);
                let is_grouped = self.state.group_by_state.is_active();
                if is_up {
                    if was_active && self.state.sort_state.direction == Some(SortDirection::Ascending) {
                        self.state.sort_state = SortState::default();
                        if !is_grouped { let _ = self.restore_original_data(); }
                    } else {
                        self.state.sort_state = SortState {
                            column: Some(col_idx),
                            direction: Some(SortDirection::Ascending),
                        };
                    }
                } else {
                    if was_active && self.state.sort_state.direction == Some(SortDirection::Descending) {
                        self.state.sort_state = SortState::default();
                        if !is_grouped { let _ = self.restore_original_data(); }
                    } else {
                        self.state.sort_state = SortState {
                            column: Some(col_idx),
                            direction: Some(SortDirection::Descending),
                        };
                    }
                }
                return;
            }
            return;
        }

        // No modifier keys and no multi-selection — plain cell clicks are handled
        // by on_drag_end already, so skip here to avoid double-handling.
        if !shift && !ctrl {
            return;
        }

        let effective = self.state.effective_columns();
        let h = self.state.height;
        let rh = self.state.row_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
        let tx = self.state.translate_x;
        let ty = self.state.translate_y;
        let ftr = self.state.freeze_trailing_rows;
        let has_groups = self.state.enable_groups;

        let hit = hit_test::hit_test(
            x, y, &effective, h, h, has_groups, hh, ghh, rows, rh,
            self.state.cell_x_offset, cyo, tx, ty, ftr,
        );

        match hit {
            None => {}
            Some((col, row)) => {
                if ctrl {
                    let item = types::Item::new(col, row);
                    if self.state.selection.current.is_none() {
                        self.state.selection = types::GridSelection::single(col, row);
                    } else {
                        let already = self.state.selection.ctrl_cells.iter().position(|i| i == &item);
                        if let Some(idx) = already {
                            self.state.selection.ctrl_cells.remove(idx);
                        } else {
                            self.state.selection.ctrl_cells.push(item);
                        }
                    }
                } else if shift {
                    if let Some(ref cur) = self.state.selection.current {
                        let anchor_col = cur.cell.col;
                        let anchor_row = cur.cell.row;
                        let min_col = anchor_col.min(col) as f64;
                        let max_col = anchor_col.max(col) as f64;
                        let min_row = anchor_row.min(row) as f64;
                        let max_row = anchor_row.max(row) as f64;
                        let new_range = types::Rectangle::new(
                            min_col, min_row,
                            max_col - min_col + 1.0,
                            max_row - min_row + 1.0,
                        );
                        self.state.selection.current.as_mut().unwrap().range = new_range;
                        self.state.selection.ctrl_cells.clear();
                    } else {
                        self.state.selection = types::GridSelection::single(col, row);
                    }
                }
            }
        }
    }

    pub fn on_mouse_move(&mut self, x: f64, y: f64) -> String {
                // Column drag tracking
        if self.state.col_drag.is_some() {
            {
                let cd = self.state.col_drag.as_mut().unwrap();
                cd.prev_mouse_x = cd.mouse_x;
                cd.mouse_x = x;
                cd.mouse_y = y;
                if !cd.has_activated {
                    let dx = (x - cd.start_x).abs();
                    if dx > COL_DRAG_ACTIVATE_PX {
                        cd.has_activated = true;
                    }
                }
            }

            let activated = self.state.col_drag.as_ref().unwrap().has_activated;
            if activated {
                if let Some((a, b)) = self.detect_col_swap(x) {
                    self.perform_col_swap(a, b);
                    return "col-swap".to_string();
                }
            }
            return "grabbing".to_string();
        }

        // Clean up finished animations lazily
        self.state.clean_finished_animation();

        if let Some(ref mut rs) = self.state.resize_state {
            rs.current_x = x;
            return "col-resize".to_string();
        }

                if let Some(col_idx) = layout::hit_test_resize_border(
            x, y, &self.state.col_layout,
        ) {
            self.state.resize_hover_col = Some(col_idx);
            return "col-resize".to_string();
        }

        self.state.resize_hover_col = None;
        "default".to_string()
    }

    fn detect_col_swap(&self, mouse_x: f64) -> Option<(usize, usize)> {
        let cd = self.state.col_drag.as_ref()?;
        let drag_col = cd.col_display_index;
        let moving_right = cd.mouse_x - cd.prev_mouse_x >= 0.0;

        let mapped = &self.state.mapped_columns;
        let tx = self.state.translate_x;

        let mut col_positions: Vec<(usize, f64, f64)> = Vec::new();
        let mut x_acc = 0.0f64;
        for c in mapped {
            let draw_x = if c.sticky { x_acc } else { x_acc + tx };
            col_positions.push((c.source_index, draw_x, c.width));
            x_acc += c.width;
        }

        let drag_pos = col_positions.iter().position(|(si, _, _)| *si == drag_col)?;

        // Helper: check whether a display index belongs to a group-by key column.
        let is_group_key = |display_idx: usize| -> bool {
            if !self.state.group_by_state.is_active() { return false; }
            self.state.resolved_columns.as_ref()
                .and_then(|r| r.leaf_by_display_index(display_idx))
                .map(|l| self.state.group_by_state.has_source_column(&l.arrow_name))
                .unwrap_or(false)
        };

        if moving_right && drag_pos + 1 < col_positions.len() {
            let (neighbor_si, neighbor_x, neighbor_w) = col_positions[drag_pos + 1];
            // In grouped mode: group keys can only swap with group keys,
            // value columns can only swap with value columns.
            if is_group_key(drag_col) == is_group_key(neighbor_si) {
                let threshold = neighbor_x + neighbor_w * COL_SWAP_THRESHOLD_FORWARD;
                if mouse_x > threshold {
                    return Some((drag_col, neighbor_si));
                }
            }
        }

        if !moving_right && drag_pos > 0 {
            let (neighbor_si, neighbor_x, neighbor_w) = col_positions[drag_pos - 1];
            if is_group_key(drag_col) == is_group_key(neighbor_si) {
                let threshold = neighbor_x + neighbor_w * COL_SWAP_THRESHOLD_BACKWARD;
                if mouse_x < threshold {
                    return Some((drag_col, neighbor_si));
                }
            }
        }

        None
    }

    fn perform_col_swap(&mut self, a: usize, b: usize) {
        use crate::types::ColSlideAnimation;

        // Fast-forward any existing animation
        self.state.col_slide_anim = None;

        let leaf_y = self.state.col_layout.leaf_y;
        let canvas_h = self.state.height;
        let capture_y = leaf_y;
        let capture_h = canvas_h - leaf_y;

        // Use the precomputed layout for pixel positions
        let a_entry = self.state.col_layout.entry_by_source(a);
        let b_entry = self.state.col_layout.entry_by_source(b);
        let a_x = a_entry.map(|e| e.draw_x).unwrap_or(0.0);
        let a_w = a_entry.map(|e| e.width).unwrap_or(FALLBACK_COLUMN_WIDTH);
        let b_x = b_entry.map(|e| e.draw_x).unwrap_or(0.0);
        let b_w = b_entry.map(|e| e.width).unwrap_or(FALLBACK_COLUMN_WIDTH);

        // Capture pre-swap column strips from the canvas (owned by DataGrid, not GridState)
        let canvas_a = self.canvas.capture_rect(a_x, capture_y, a_w, capture_h).ok();
        let canvas_b = self.canvas.capture_rect(b_x, capture_y, b_w, capture_h).ok();

        // Commit the swap
        self.state.swap_columns(a, b);

        // Update drag state
        if let Some(ref mut cd) = self.state.col_drag {
            if cd.col_display_index == a {
                cd.col_display_index = b;
            } else if cd.col_display_index == b {
                cd.col_display_index = a;
            }
        }

        // Render post-swap state (headers + data all correct)
        self.do_render();

        // Start slide animation (the captured pre-swap strips slide to new positions)
        if let (Some(ca), Some(cb)) = (canvas_a, canvas_b) {
            self.state.col_slide_anim = Some(ColSlideAnimation {
                canvas_a: ca,
                canvas_b: cb,
                a_start_x: a_x,
                b_start_x: b_x,
                a_end_x: b_x,
                b_end_x: a_x,
                y: capture_y,
                start_time_ms: js_sys::Date::now(),
                duration_ms: self.state.swap_animation_duration_ms,
            });
        }
    }

    pub fn is_animating(&self) -> bool {
        self.state.col_slide_anim.as_ref()
            .map_or(false, |anim| !anim.is_done(js_sys::Date::now()))
    }

    /// Returns the display index of the column whose resize border last triggered
    /// the col-resize cursor, or -1 if none.
    pub fn get_resize_hover_col(&self) -> i32 {
        self.state.resize_hover_col.map(|c| c as i32).unwrap_or(-1)
    }

    // --- Grouping / Aggregation ---

    /// Returns JSON with the context needed to populate the column header menu.
    pub fn get_column_menu_context(&self, display_index: usize) -> String {
        let is_grouped = self.state.group_by_state.is_active();

        // Find the leaf's arrow_name (in grouped mode this is e.g. "price_count")
        let leaf_arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .or_else(|| self.state.columns.get(display_index).map(|c| c.title.clone()))
            .unwrap_or_default();

        let display_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.display_name.clone())
            .or_else(|| self.state.columns.get(display_index).map(|c| c.title.clone()))
            .unwrap_or_default();

        // In grouped mode, reverse-map "price_count" → "price" (the original col name)
        // by checking group_keys and aggregation entries
        let mut this_agg_fn: Option<String> = None;
        let original_col_name = if is_grouped {
            // Check if leaf is a raw (non-truncated) group key or a truncated date group key
            if self.state.group_by_state.has_exact_key(&leaf_arrow_name, None)
                || self.state.group_by_state.group_keys.iter()
                    .any(|k| k.result_name() == leaf_arrow_name)
            {
                // It's a group key (raw or truncated); find the source column name
                self.state.group_by_state.group_keys.iter()
                    .find(|k| k.result_name() == leaf_arrow_name || k.arrow_name == leaf_arrow_name)
                    .map(|k| k.arrow_name.clone())
                    .unwrap_or_else(|| leaf_arrow_name.clone())
            } else {
                // Find which original column this aggregated column belongs to
                // by checking if leaf_arrow_name matches "{col}_{fn}" pattern
                let mut found = None;
                'outer: for (col_name, fns) in &self.state.group_by_state.aggregations {
                    for f in fns {
                        if f.alias(col_name) == leaf_arrow_name {
                            found = Some(col_name.clone());
                            this_agg_fn = Some(format!("{:?}", f).to_lowercase());
                            break 'outer;
                        }
                    }
                }
                found.unwrap_or_else(|| leaf_arrow_name.clone())
            }
        } else {
            leaf_arrow_name.clone()
        };

        let is_group_key = self.state.group_by_state.has_source_column(&original_col_name)
            && self.state.group_by_state.group_keys.iter()
                .any(|k| k.result_name() == leaf_arrow_name || (k.arrow_name == original_col_name && k.truncation.is_none()));

        let current_aggs: Vec<String> = self.state.group_by_state
            .agg_fns_for(&original_col_name)
            .unwrap_or(&[])
            .iter()
            .map(|f| format!("{:?}", f).to_lowercase())
            .collect();

        // Look up type from ORIGINAL data schema (not grouped result)
        let compatible_aggs: Vec<String> = if let Some(data) = self.state.arrow_data.as_ref() {
            let dtype = data.schema().fields().iter()
                .find(|f| f.name() == &original_col_name)
                .map(|f| f.data_type().clone())
                .unwrap_or(arrow_schema::DataType::Utf8);
            let type_compat = AggregateFunction::compatible_with(&dtype);

            // Filter through availableAggregateFunctions
            let filtered: Vec<AggregateFunction> = match &self.state.available_agg_functions {
                None => type_compat,
                Some(types::AvailableAggFunctions::Global(list)) =>
                    type_compat.into_iter().filter(|f| list.contains(f)).collect(),
                Some(types::AvailableAggFunctions::PerColumn(map)) => {
                    if let Some(col_list) = map.get(&original_col_name) {
                        type_compat.into_iter().filter(|f| col_list.contains(f)).collect()
                    } else {
                        type_compat
                    }
                }
            };
            filtered.iter().map(|f| format!("{:?}", f).to_lowercase()).collect()
        } else {
            vec!["count".to_string()]
        };

        // Whether this column can be added as a group-by key
        // (allowed in both grouped and non-grouped mode, as long as it's not already a key)
        let can_group_by = !is_group_key && {
            match &self.state.allowable_group_by {
                None => true,
                Some(allowed) => allowed.contains(&original_col_name),
            }
        };

        // Whether this is a mandatory group-by key (cannot be removed)
        let is_mandatory_group_key = self.state.mandatory_group_by.contains(&original_col_name);

        // The parent display name (original column name shown above the agg leaf)
        let parent_display = if is_grouped && !is_group_key {
            self.state.resolved_columns.as_ref()
                .and_then(|r| r.leaf_by_display_index(display_index))
                .and_then(|l| l.parent_titles.last().cloned())
                .unwrap_or_else(|| original_col_name.clone())
        } else {
            display_name.clone()
        };

        let sibling_agg_count = self.state.group_by_state
            .agg_fns_for(&original_col_name)
            .map(|fns| fns.len())
            .unwrap_or(0);

        serde_json::json!({
            "display_index": display_index,
            "arrow_name": original_col_name,
            "display_name": parent_display,
            "is_group_key": is_group_key,
            "is_grouped_mode": is_grouped,
            "current_aggs": current_aggs,
            "compatible_aggs": compatible_aggs,
            "this_agg_fn": this_agg_fn,
            "sibling_agg_count": sibling_agg_count,
            "can_group_by": can_group_by,
            "is_mandatory_group_key": is_mandatory_group_key,
        }).to_string()
    }

    /// Get the display index of the column whose menu button was last clicked.
    pub fn get_last_menu_col(&self) -> i32 {
        self.state.last_menu_col.map(|c| c as i32).unwrap_or(-1)
    }

    /// Toggle a raw (non-truncated) column as a group-by key.
    pub async fn toggle_group_key(&mut self, arrow_name: &str) -> Result<(), JsValue> {
        self.toggle_group_key_truncated(arrow_name, "null").await
    }

    /// Toggle a date column as a group-by key with a specific truncation level.
    /// truncation: "month", "year", "day", etc. or "null" for no truncation.
    pub async fn toggle_group_key_truncated(&mut self, arrow_name: &str, truncation: &str) -> Result<(), JsValue> {
        use types::DateGroupKey;

        let trunc: Option<types::DateTruncation> = if truncation == "null" || truncation.is_empty() {
            None
        } else {
            Some(serde_json::from_value(serde_json::json!(truncation))
                .map_err(|_| JsValue::from_str(&format!("Unknown truncation: {}", truncation)))?)
        };

        let key = DateGroupKey { arrow_name: arrow_name.to_string(), truncation: trunc };
        let is_key = self.state.group_by_state.group_keys.contains(&key);

        if is_key {
            // Silently ignore removal requests for mandatory columns (only for raw keys)
            if trunc.is_none() && self.state.mandatory_group_by.contains(&arrow_name.to_string()) {
                return Ok(());
            }

            self.state.group_by_state.group_keys.retain(|k| k != &key);

            // Raw key removal: add back to aggregations
            if trunc.is_none() {
                self.state.group_by_state.aggregations.retain(|(n, _)| n != arrow_name);
            }
            // Truncated key removal: source column stays in aggregations (already there)

            // After removal, ensure mandatory columns remain as raw group keys
            for mandatory in &self.state.mandatory_group_by.clone() {
                if !self.state.group_by_state.has_exact_key(mandatory, None) {
                    self.state.group_by_state.group_keys.push(DateGroupKey::raw(mandatory));
                    self.state.group_by_state.aggregations.retain(|(n, _)| n != mandatory.as_str());
                }
            }

            if self.state.group_by_state.group_keys.is_empty() {
                self.state.exit_group_by();
                return Ok(());
            }
        } else {
            // Check allowable_group_by restriction
            if let Some(ref allowed) = self.state.allowable_group_by {
                if !allowed.contains(&arrow_name.to_string()) {
                    return Err(JsValue::from_str(&format!(
                        "Column '{}' is not in allowableGroupBy", arrow_name
                    )));
                }
            }
            self.state.group_by_state.group_keys.push(key.clone());
            // Raw key: remove from aggregations (column is now a group key, not a value)
            // Truncated key: leave in aggregations (source column stays as value)
            if trunc.is_none() {
                self.state.group_by_state.aggregations.retain(|(n, _)| n != arrow_name);
            }

            // Auto-apply a default DateTime format for newly added truncated keys,
            // stripping components already covered by sibling keys on the same source column.
            if let Some(t) = trunc {
                let fmt = compute_default_truncation_format(
                    arrow_name,
                    t,
                    &self.state.group_by_state.group_keys,
                );
                self.state.format_overrides.insert(key.result_name(), Some(fmt));
            }
        }

        // Snapshot keys/aggs for auto-expand BEFORE enter_group_by (they won't change but
        // borrow checker needs these to be independent of &mut self below).
        let n_keys_after = self.state.group_by_state.group_keys.len();
        let should_auto_expand = !is_key && n_keys_after > 1;
        let auto_expand_group_keys = self.state.group_by_state.group_keys.clone();
        let auto_expand_aggregations = self.state.group_by_state.aggregations.clone();

        self.state.enter_group_by(DEFAULT_COLUMN_WIDTH).await
            .map_err(|e| JsValue::from_str(&e))?;

        // Auto-expand: after adding a new group key, run one combined group_by over all keys,
        // partition the result by the first key's value, and mark every top-level row expanded.
        if should_auto_expand {
            let first_key = &auto_expand_group_keys[0];
            let first_result_name = first_key.result_name();

            let combined = if let Some(ref data) = self.state.arrow_data {
                data.group_by(&auto_expand_group_keys, &auto_expand_aggregations).await.ok()
            } else {
                None
            };

            if let Some(all_keys_data) = combined {
                let col_idx = all_keys_data.schema().fields().iter()
                    .position(|f| f.name().as_str() == first_result_name.as_str())
                    .unwrap_or(0);

                let partitions = all_keys_data.partition_by_column(col_idx);

                for (key_display_val, sub_data) in partitions {
                    let cache_key: types::ExpandCacheKey =
                        vec![(first_result_name.clone(), key_display_val)];
                    self.state.expand_caches.insert(cache_key.clone(), sub_data);
                    self.state.expanded_keys.insert(cache_key);
                }

                self.state.rebuild_virtual_rows();
            }
        }

        Ok(())
    }

    /// Remove all group keys (any truncation) for the given source column.
    /// If no group keys remain afterwards, exits group-by mode entirely.
    pub async fn clear_group_key(&mut self, arrow_name: &str) -> Result<(), JsValue> {
        self.state.group_by_state.group_keys.retain(|k| k.arrow_name != arrow_name);

        if self.state.group_by_state.group_keys.is_empty() {
            self.state.exit_group_by();
            return Ok(());
        }

        self.state.enter_group_by(DEFAULT_COLUMN_WIDTH).await
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Set aggregation functions for a value column. Pass empty array to remove all.
    /// If removing all and this is the only aggregated column, keeps count.
    pub async fn set_column_aggregations(&mut self, arrow_name: &str, agg_fns_json: &str) -> Result<(), JsValue> {
        let fns: Vec<String> = serde_json::from_str(agg_fns_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid JSON: {}", e)))?;

        let parsed: Vec<AggregateFunction> = fns.iter().filter_map(|s| match s.as_str() {
            "count" => Some(AggregateFunction::Count),
            "sum" => Some(AggregateFunction::Sum),
            "min" => Some(AggregateFunction::Min),
            "max" => Some(AggregateFunction::Max),
            "mean" => Some(AggregateFunction::Mean),
            _ => None,
        }).collect();

        // Don't allow removing all aggs from a column if it would leave nothing aggregated
        let final_fns = if parsed.is_empty() {
            vec![AggregateFunction::Count]
        } else {
            parsed
        };

        self.state.group_by_state.set_agg_fns(arrow_name, final_fns);
        self.state.run_group_by_query_preserve_expand(DEFAULT_COLUMN_WIDTH).await
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Replace one specific agg function with another for a column.
    /// E.g. replace Sum with Min for "cost", while keeping Mean intact.
    pub async fn replace_column_aggregation(
        &mut self,
        arrow_name: &str,
        old_fn: &str,
        new_fn: &str,
    ) -> Result<(), JsValue> {
        let parse_fn = |s: &str| match s {
            "count" => Some(AggregateFunction::Count),
            "sum" => Some(AggregateFunction::Sum),
            "min" => Some(AggregateFunction::Min),
            "max" => Some(AggregateFunction::Max),
            "mean" => Some(AggregateFunction::Mean),
            _ => None,
        };
        let new_agg = parse_fn(new_fn)
            .ok_or_else(|| JsValue::from_str(&format!("Unknown aggregation: {}", new_fn)))?;

        let current = self.state.group_by_state
            .agg_fns_for(arrow_name)
            .unwrap_or(&[])
            .to_vec();

        let updated: Vec<AggregateFunction> = current.into_iter()
            .map(|f| {
                if format!("{:?}", f).to_lowercase() == old_fn { new_agg.clone() } else { f }
            })
            .collect();

        self.state.group_by_state.set_agg_fns(arrow_name, updated);
        self.state.run_group_by_query_preserve_expand(DEFAULT_COLUMN_WIDTH).await
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Get context for the last-clicked parent span ⋮ button.
    /// Returns JSON: { arrow_name, display_name, current_aggs, available_to_add }
    pub fn get_span_menu_context(&self) -> String {
        let (level_idx, first_leaf, last_leaf) = match self.state.last_span_menu {
            Some(s) => s,
            None => return "null".to_string(),
        };

        // Find which original column this span represents by looking at leaves
        // in the range [first_leaf, last_leaf] and finding their common parent at this level.
        let resolved = match &self.state.resolved_columns {
            Some(r) => r,
            None => return "null".to_string(),
        };

        // Get the span's title from header_levels
        let span_title = resolved.header_levels
            .get(level_idx)
            .and_then(|spans| {
                spans.iter().find(|s| s.first_leaf == first_leaf && s.last_leaf == last_leaf)
            })
            .map(|s| s.title.clone())
            .unwrap_or_default();

        // In grouped mode, parent span titles are either "Group" or the original column name.
        // Find the original arrow_name by matching the span title against column display names.
        let arrow_name = self.state.group_by_state.aggregations.iter()
            .find(|(col_name, _)| {
                // Check if this column's display name matches span_title
                if let Some(ref orig_inputs) = self.state.original_column_input_snapshot {
                    fn find_display(inputs: &[crate::types::ColumnInput], name: &str) -> Option<String> {
                        for input in inputs {
                            if input.name.as_deref() == Some(name) {
                                return input.display.clone().or_else(|| Some(name.to_string()));
                            }
                            if let Some(ref children) = input.children {
                                if let Some(d) = find_display(children, name) {
                                    return Some(d);
                                }
                            }
                        }
                        None
                    }
                    find_display(orig_inputs, col_name)
                        .map(|d| d == span_title)
                        .unwrap_or(false)
                } else {
                    col_name == &span_title
                }
            })
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| span_title.clone());

        let current_aggs: Vec<String> = self.state.group_by_state
            .agg_fns_for(&arrow_name)
            .unwrap_or(&[])
            .iter()
            .map(|f| format!("{:?}", f).to_lowercase())
            .collect();

        // Compatible agg functions from original data schema
        let compatible: Vec<String> = if let Some(data) = self.state.arrow_data.as_ref() {
            let dtype = data.schema().fields().iter()
                .find(|f| f.name() == &arrow_name)
                .map(|f| f.data_type().clone())
                .unwrap_or(arrow_schema::DataType::Utf8);
            AggregateFunction::compatible_with(&dtype)
                .iter()
                .map(|f| format!("{:?}", f).to_lowercase())
                .collect()
        } else {
            vec!["count".to_string()]
        };

        // available_to_add = compatible minus already active
        let available_to_add: Vec<String> = compatible.iter()
            .filter(|f| !current_aggs.contains(f))
            .cloned()
            .collect();

        serde_json::json!({
            "arrow_name": arrow_name,
            "display_name": span_title,
            "current_aggs": current_aggs,
            "available_to_add": available_to_add,
        }).to_string()
    }

    /// Exit grouped mode entirely, restoring original data and columns.
    pub fn clear_grouping(&mut self) {
        self.state.exit_group_by();
    }

    /// Set the columns that are allowed to be used as group-by keys.
    /// json: JSON array of arrow_name strings, or "null" to allow all.
    pub fn set_allowable_group_by(&mut self, json: &str) -> Result<(), JsValue> {
        if json == "null" {
            self.state.allowable_group_by = None;
        } else {
            let names: Vec<String> = serde_json::from_str(json)
                .map_err(|e| JsValue::from_str(&format!("Invalid allowableGroupBy: {}", e)))?;
            self.state.allowable_group_by = Some(names);
        }
        Ok(())
    }

    /// Set the columns that are always group-by keys (cannot be removed by the user).
    /// json: JSON array of arrow_name strings.
    pub fn set_mandatory_group_by(&mut self, json: &str) -> Result<(), JsValue> {
        let names: Vec<String> = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Invalid mandatoryGroupBy: {}", e)))?;
        self.state.mandatory_group_by = names;
        Ok(())
    }

    /// Set which aggregate functions are available in the column ⋮ menu.
    /// json: either a JSON array (global) or a JSON object (per-column by arrow_name).
    pub fn set_available_aggregate_functions(&mut self, json: &str) -> Result<(), JsValue> {
        if json == "null" {
            self.state.available_agg_functions = None;
            return Ok(());
        }
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Invalid availableAggregateFunctions: {}", e)))?;
        let parsed: types::AvailableAggFunctions = serde_json::from_value(value)
            .map_err(|e| JsValue::from_str(&format!("Invalid availableAggregateFunctions: {}", e)))?;
        self.state.available_agg_functions = Some(parsed);
        Ok(())
    }

    /// Set available date truncation levels. Same pattern as availableAggregateFunctions.
    /// json: array of level strings (global) or object (per column by arrow_name).
    pub fn set_available_date_truncations(&mut self, json: &str) -> Result<(), JsValue> {
        if json == "null" {
            self.state.available_date_truncations = None;
            return Ok(());
        }
        let parsed: types::AvailableDateTruncations = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Invalid availableDateTruncations: {}", e)))?;
        self.state.available_date_truncations = Some(parsed);
        Ok(())
    }

    /// Get date truncation options for a column. Triggers inference if not cached.
    /// Returns JSON: { available: [...], active: [...], is_estimate: bool }
    pub fn get_date_truncation_options(&mut self, display_index: usize) -> String {
        let leaf_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .unwrap_or_default();
        if leaf_name.is_empty() { return "null".to_string(); }

        // If the leaf is a truncated group key result (e.g. "created_at_month"), resolve
        // back to the source column name so we can look it up in the schema and cache.
        let arrow_name = self.state.group_by_state.group_keys.iter()
            .find(|k| k.result_name() == leaf_name)
            .map(|k| k.arrow_name.clone())
            .unwrap_or(leaf_name);

        // Check if this is actually a date column — native types OR Utf8 columns
        // that are in the truncation cache (ISO 8601 strings detected at data load).
        let dtype = self.state.arrow_data.as_ref()
            .and_then(|d| d.schema().fields().iter().find(|f| f.name() == &arrow_name).map(|f| f.data_type().clone()))
            .or_else(|| self.state.grouped_data.as_ref()
                .and_then(|d| d.schema().fields().iter().find(|f| f.name() == &arrow_name).map(|f| f.data_type().clone())));

        let is_date = match &dtype {
            Some(dt) => matches!(dt,
                arrow_schema::DataType::Date32 | arrow_schema::DataType::Date64 |
                arrow_schema::DataType::Timestamp(_, _)),
            None => false,
        } || {
            // Utf8 column that's already in the cache (detected as date-like at load time)
            self.state.date_truncation_cache.contains_key(&arrow_name)
        };

        if !is_date { return "null".to_string(); }

        self.state.ensure_date_truncation_options(&arrow_name);

        let opts = match self.state.date_truncation_cache.get(&arrow_name) {
            Some(o) => o,
            None => return "null".to_string(),
        };

        let available: Vec<&str> = opts.available.iter().map(|t| t.precision()).collect();
        let active: Vec<String> = self.state.group_by_state.keys_for_source(&arrow_name)
            .iter()
            .filter_map(|k| k.truncation.map(|t| t.precision().to_string()))
            .collect();

        serde_json::json!({
            "arrow_name": arrow_name,
            "available": available,
            "active": active,
            "is_estimate": opts.is_estimate,
        }).to_string()
    }

    /// Whether grouping is currently active.
    pub fn is_grouped(&self) -> bool {
        self.state.group_by_state.is_active()
    }

    /// Returns JSON: current group-by state { group_keys, aggregations }.
    pub fn get_group_by_state(&self) -> String {
        serde_json::to_string(&self.state.group_by_state).unwrap_or_else(|_| "{}".to_string())
    }

    // --- Format picker ---

    /// Returns JSON with format options for a column based on its Arrow data type.
    /// { current_format: {...}|null, compatible_formats: [{label, spec: {...}}] }
    pub fn get_format_options(&self, display_index: usize) -> String {
        use arrow_schema::DataType;

        let arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .unwrap_or_default();

        // Look up data type from original schema, then grouped schema as fallback.
        let dtype = self.state.arrow_data.as_ref()
            .and_then(|d| d.schema().fields().iter().find(|f| f.name() == &arrow_name).map(|f| f.data_type().clone()))
            .or_else(|| self.state.grouped_data.as_ref()
                .and_then(|d| d.schema().fields().iter().find(|f| f.name() == &arrow_name).map(|f| f.data_type().clone())))
            .unwrap_or(DataType::Utf8);

        // For Utf8 columns that are derived from a date-like source (e.g. created_at_min),
        // resolve the source column name by stripping known aggregation suffixes.
        // If the source is in the date_truncation_cache (populated when the user opened the
        // date group-by submenu or after a sort), treat this column as datetime.
        let is_date_like_utf8 = matches!(dtype, DataType::Utf8 | DataType::LargeUtf8) && {
            // Try the arrow_name itself, then strip known agg suffixes to find source
            let candidates: Vec<String> = {
                let mut c = vec![arrow_name.clone()];
                for suffix in &["_min", "_max", "_count", "_sum", "_mean", "_year", "_month",
                                "_quarter", "_week", "_day", "_hour", "_minute", "_second"] {
                    if arrow_name.ends_with(suffix) {
                        c.push(arrow_name[..arrow_name.len() - suffix.len()].to_string());
                    }
                }
                c
            };
            candidates.iter().any(|name| self.state.date_truncation_cache.contains_key(name))
        };

        let current_format = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .and_then(|l| l.data_style.as_ref())
            .and_then(|ds| ds.number_format.as_ref())
            .and_then(|f| serde_json::to_value(f).ok());

        let mut formats: Vec<serde_json::Value> = Vec::new();

        let datetime_formats = || vec![
            serde_json::json!({"label": "Default",              "spec": null}),
            serde_json::json!({"label": "YYYY-MM-DD HH:MM:SS", "spec": {"type": "dateTime", "format": "%Y-%m-%d %H:%M:%S"}}),
            serde_json::json!({"label": "YYYY-MM-DD",          "spec": {"type": "dateTime", "format": "%Y-%m-%d"}}),
            serde_json::json!({"label": "YYYY-MM",             "spec": {"type": "dateTime", "format": "%Y-%m"}}),
            serde_json::json!({"label": "YYYY",                "spec": {"type": "dateTime", "format": "%Y"}}),
            serde_json::json!({"label": "MM/DD/YYYY HH:MM",    "spec": {"type": "dateTime", "format": "%m/%d/%Y %H:%M"}}),
            serde_json::json!({"label": "HH:MM:SS",            "spec": {"type": "dateTime", "format": "%H:%M:%S"}}),
            serde_json::json!({"label": "HH:MM",               "spec": {"type": "dateTime", "format": "%H:%M"}}),
            serde_json::json!({"label": "DD",                  "spec": {"type": "dateTime", "format": "%d"}}),
            serde_json::json!({"label": "HH",                  "spec": {"type": "dateTime", "format": "%H"}}),
            serde_json::json!({"label": "ISO 8601",            "spec": {"type": "dateTime", "format": "%Y-%m-%dT%H:%M:%SZ"}}),
        ];

        let is_date;
        match &dtype {
            DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64
            | DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64
            | DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                is_date = false;
                formats.extend([
                    serde_json::json!({"label": "Default",      "spec": null,                                                "has_decimals": false}),
                    serde_json::json!({"label": "Number",       "spec": {"type": "decimal",    "decimals": 2},             "has_decimals": true}),
                    serde_json::json!({"label": "Integer",      "spec": {"type": "integer"},                               "has_decimals": false}),
                    serde_json::json!({"label": "Currency $",   "spec": {"type": "currency",   "symbol": "$", "decimals": 2}, "has_decimals": true}),
                    serde_json::json!({"label": "Accounting $", "spec": {"type": "accounting", "decimals": 2},             "has_decimals": true}),
                    serde_json::json!({"label": "Percent",      "spec": {"type": "percent",    "decimals": 1},             "has_decimals": true}),
                ]);
            }
            DataType::Date32 | DataType::Date64 => {
                is_date = true;
                formats.extend([
                    serde_json::json!({"label": "Default",          "spec": null}),
                    serde_json::json!({"label": "YYYY-MM-DD",       "spec": {"type": "date", "format": "%Y-%m-%d"}}),
                    serde_json::json!({"label": "MM/DD/YYYY",       "spec": {"type": "date", "format": "%m/%d/%Y"}}),
                    serde_json::json!({"label": "DD/MM/YYYY",       "spec": {"type": "date", "format": "%d/%m/%Y"}}),
                    serde_json::json!({"label": "MMM D, YYYY",      "spec": {"type": "date", "format": "%b %-d, %Y"}}),
                    serde_json::json!({"label": "MMMM D, YYYY",     "spec": {"type": "date", "format": "%B %-d, %Y"}}),
                ]);
            }
            DataType::Timestamp(_, _) => {
                is_date = true;
                formats.extend(datetime_formats());
            }
            _ if is_date_like_utf8 => {
                // Utf8 column that's known to contain ISO 8601 datetimes (e.g. created_at,
                // created_at_min, created_at_max). Offer datetime formatting options.
                is_date = true;
                formats.extend(datetime_formats());
            }
            _ => {
                is_date = false;
            }
        }

        serde_json::json!({
            "display_index": display_index,
            "arrow_name": arrow_name,
            "current_format": current_format,
            "compatible_formats": formats,
            "is_numeric": matches!(dtype,
                DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64
                | DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64
                | DataType::Float16 | DataType::Float32 | DataType::Float64),
            "is_date": is_date,
        }).to_string()
    }

    /// Set a display format override for a column.
    /// format_json: serialized NumberFormat, or "null" to clear.
    pub fn set_column_format(&mut self, display_index: usize, format_json: &str) -> Result<(), JsValue> {
        let arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .unwrap_or_default();

        if arrow_name.is_empty() {
            return Err(JsValue::from_str("Column not found"));
        }

        let fmt: Option<types::NumberFormat> = if format_json == "null" {
            None
        } else {
            Some(serde_json::from_str(format_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid format JSON: {}", e)))?)
        };

        self.state.apply_format_override(&arrow_name, fmt);
        Ok(())
    }

    // --- Copy to clipboard ---

    /// Returns TSV (tab-separated values) of the current selection.
    /// Numbers returned as raw floats, dates as ISO strings.
    pub fn get_selected_cells_tsv(&self) -> String {
        let selection = &self.state.selection;

        let get_raw = |col: i32, row: i32| -> String {
            if col < 0 || row < 0 { return String::new(); }
            let arrow_col = self.state.display_to_arrow_index(col as usize);
            let data = match self.state.grouped_data.as_ref().or(self.state.arrow_data.as_ref()) {
                Some(d) => d,
                None => return String::new(),
            };
            if row as usize >= data.num_rows() || arrow_col >= data.num_columns() {
                return String::new();
            }
            data.get_cell_raw_text(arrow_col, row as usize)
        };

        if let Some(ref cur) = selection.current {
            let range = &cur.range;
            if range.width == 1.0 && range.height == 1.0 && selection.ctrl_cells.is_empty() {
                // Single cell
                return get_raw(cur.cell.col, cur.cell.row);
            }

            // Multi-cell range: rows × cols as TSV
            if !selection.ctrl_cells.is_empty() {
                // Non-contiguous: one cell per line
                let mut lines: Vec<String> = Vec::new();
                // Include the anchor cell
                lines.push(get_raw(cur.cell.col, cur.cell.row));
                for item in &selection.ctrl_cells {
                    lines.push(get_raw(item.col, item.row));
                }
                return lines.join("\n");
            }

            // Rectangular range
            let min_col = range.x as i32;
            let max_col = (range.x + range.width - 1.0) as i32;
            let min_row = range.y as i32;
            let max_row = (range.y + range.height - 1.0) as i32;

            let mut rows: Vec<String> = Vec::new();
            for row in min_row..=max_row {
                let mut cols: Vec<String> = Vec::new();
                for col in min_col..=max_col {
                    cols.push(get_raw(col, row));
                }
                rows.push(cols.join("\t"));
            }
            return rows.join("\n");
        }

        String::new()
    }

    // --- Scroll metrics ---

    /// Returns JSON with scroll metrics needed for scrollbar rendering.
    /// Returns JSON array of layout entries for testing/debugging hit areas.
    ///
    /// Each entry has the structure:
    /// {
    ///   "col": <source_index>,
    ///   "col_draw_x": <left pixel edge of the column>,
    ///   "col_width": <column pixel width>,
    ///   "right_border_x": <pixel x of the column's right border line>,
    ///   "leaf_y": <pixel y of the top of the leaf header row>,
    ///   "leaf_h": <pixel height of the leaf header row>,
    ///
    ///   "menu_btn": {
    ///     // The ⋮ button: three vertical dots drawn at draw_x_center
    ///     "draw_x_min":    <left edge of drawn dots bounding box>,
    ///     "draw_x_max":    <right edge of drawn dots bounding box>,
    ///     "draw_x_center": <horizontal center of the drawn dots>,
    ///     "hit_x_min":     <leftmost x the click is detected>,
    ///     "hit_x_max":     <rightmost x the click is detected>
    ///   },
    ///
    ///   "sort_triangles": {
    ///     // Both ▲ and ▼ share the same x hit range; y split discriminates which was clicked
    ///     "draw_x_min":    <left edge of drawn triangle bounding box>,
    ///     "draw_x_max":    <right edge of drawn triangle bounding box (= draw_x_min + 7)>,
    ///     "draw_x_center": <horizontal center of the triangle bounding box>,
    ///     "hit_x_min":     <leftmost x the click is detected (= menu_btn.hit_x_max)>,
    ///     "hit_x_max":     <rightmost x the click is detected (= resize.hit_x_min)>,
    ///     "up": {
    ///       "draw_y_min":    <top of drawn up-triangle>,
    ///       "draw_y_max":    <bottom of drawn up-triangle (= draw_y_min + 7)>,
    ///       "draw_y_center": <vertical center of drawn up-triangle>,
    ///       "hit_y_min":     <topmost y the click is detected>,
    ///       "hit_y_max":     <bottommost y the click is detected (adjacent to down.hit_y_min)>
    ///     },
    ///     "down": {
    ///       "draw_y_min":    <top of drawn down-triangle>,
    ///       "draw_y_max":    <bottom of drawn down-triangle>,
    ///       "draw_y_center": <vertical center of drawn down-triangle>,
    ///       "hit_y_min":     <topmost y the click is detected (= up.hit_y_max)>,
    ///       "hit_y_max":     <bottommost y the click is detected>
    ///     }
    ///   },
    ///
    ///   "resize": {
    ///     // The column border is a 1px line; "drawn" position = the border itself
    ///     "draw_x_center": <pixel x of the column right border (same as right_border_x)>,
    ///     "hit_x_min":     <leftmost x the resize cursor activates>,
    ///     "hit_x_max":     <rightmost x the resize cursor activates>
    ///   }
    /// }
    pub fn get_layout_debug_info(&self) -> String {
        use crate::layout::RESIZE_HALF;

        let layout = &self.state.col_layout;
        let tri_size = crate::layout::tri_size();

        let entries: Vec<serde_json::Value> = layout.entries.iter().map(|e| {
            let menu_draw_half_x = crate::layout::MENU_BTN_WIDTH / 2.0;
            let menu_draw_x_min = e.menu_btn_cx - menu_draw_half_x;
            let menu_draw_x_max = e.menu_btn_cx + menu_draw_half_x;

            // triangle draw extents: left edge = tri_up_cx, right = tri_up_cx + TRI_SIZE
            let tri_draw_x_min    = e.tri_up_cx;
            let tri_draw_x_max    = e.tri_up_cx + tri_size;
            let tri_draw_x_center = e.tri_up_cx + tri_size / 2.0;

            let tri_up_rect   = &e.tri_up_rect;
            let tri_down_rect = &e.tri_down_rect;
            let menu_rect     = &e.menu_btn_rect;

            serde_json::json!({
                "col": e.source_index,
                "col_draw_x": e.draw_x,
                "col_width": e.width,
                "right_border_x": e.right_border_x,
                "leaf_y": layout.leaf_y,
                "leaf_h": layout.leaf_h,

                "menu_btn": {
                    "draw_x_min":    menu_draw_x_min,
                    "draw_x_max":    menu_draw_x_max,
                    "draw_x_center": e.menu_btn_cx,
                    "hit_x_min":     menu_rect.x,
                    "hit_x_max":     menu_rect.x + menu_rect.width,
                },

                "sort_triangles": {
                    "draw_x_min":    tri_draw_x_min,
                    "draw_x_max":    tri_draw_x_max,
                    "draw_x_center": tri_draw_x_center,
                    "hit_x_min":     tri_up_rect.x,
                    "hit_x_max":     tri_up_rect.x + tri_up_rect.width,
                    "up": {
                        "draw_y_min":    e.tri_up_cy,
                        "draw_y_max":    e.tri_up_cy + tri_size,
                        "draw_y_center": e.tri_up_cy + tri_size / 2.0,
                        "hit_y_min":     tri_up_rect.y,
                        "hit_y_max":     tri_up_rect.y + tri_up_rect.height,
                    },
                    "down": {
                        "draw_y_min":    e.tri_down_cy,
                        "draw_y_max":    e.tri_down_cy + tri_size,
                        "draw_y_center": e.tri_down_cy + tri_size / 2.0,
                        "hit_y_min":     tri_down_rect.y,
                        "hit_y_max":     tri_down_rect.y + tri_down_rect.height,
                    },
                },

                "resize": {
                    "draw_x_center": e.right_border_x,
                    "hit_x_min":     e.right_border_x - RESIZE_HALF,
                    "hit_x_max":     e.right_border_x + RESIZE_HALF,
                },
                "is_resizable": e.is_resizable,
            })
        }).collect();
        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    // --- Conditional formatting ---

    /// Apply conditional format rules to a column (by display_index).
    /// Get current conditional format rules for a column as a JSON string.
    /// Returns "[]" if no rules are set.
    pub fn get_conditional_formats(&self, display_index: usize) -> String {
        let arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .or_else(|| self.state.columns.get(display_index).map(|c| c.title.clone()))
            .unwrap_or_default();

        if let Some(rules) = self.state.conditional_format_overrides.get(&arrow_name) {
            serde_json::to_string(rules).unwrap_or_else(|_| "[]".to_string())
        } else {
            "[]".to_string()
        }
    }

    /// rules_json: JSON array of ConditionalRule, or "[]" to clear.
    pub fn set_conditional_formats(&mut self, display_index: usize, rules_json: &str) -> Result<(), JsValue> {
        let arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .or_else(|| self.state.columns.get(display_index).map(|c| c.title.clone()))
            .unwrap_or_default();

        if arrow_name.is_empty() {
            return Err(JsValue::from_str("Column not found"));
        }

        let rules: Vec<types::ConditionalRule> = serde_json::from_str(rules_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid rules JSON: {}", e)))?;

        if rules.is_empty() {
            self.state.conditional_format_overrides.remove(&arrow_name);
        } else {
            // Pre-compute column stats for any Gradient rules
            let has_gradient = rules.iter().any(|r| matches!(r, types::ConditionalRule::Gradient { .. }));
            if has_gradient {
                self.state.compute_column_stats(&arrow_name);
            }
            self.state.conditional_format_overrides.insert(arrow_name, rules);
        }
        Ok(())
    }

    /// Get sorted unique display values for a column (for value-based conditional format picker).
    /// Returns JSON: { values: [...], is_truncated: bool }
    pub fn get_column_unique_values(&self, display_index: usize) -> String {
        let arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .unwrap_or_default();

        let data = match self.state.active_data() {
            Some(d) => d,
            None => return r#"{"values":[],"is_truncated":false}"#.to_string(),
        };

        let col_idx = data.schema().fields().iter().position(|f| f.name() == &arrow_name);
        if let Some(idx) = col_idx {
            let (values, truncated) = data.column_unique_values(idx, 500);
            serde_json::json!({ "values": values, "is_truncated": truncated }).to_string()
        } else {
            r#"{"values":[],"is_truncated":false}"#.to_string()
        }
    }

    /// Get min/max stats for a numeric column (for gradient preview).
    /// Returns JSON: { min, max } or null for non-numeric / no data.
    /// Returns JSON: { min, max, p5, p95 } — true min/max and 5th/95th percentile bounds.
    /// The gradient UI uses p5/p95 as defaults since true min/max often skews the scale.
    pub fn get_column_stats(&mut self, display_index: usize) -> String {
        let arrow_name = self.state.resolved_columns.as_ref()
            .and_then(|r| r.leaf_by_display_index(display_index))
            .map(|l| l.arrow_name.clone())
            .unwrap_or_default();

        let data = match self.state.active_data() {
            Some(d) => d,
            None => return "null".to_string(),
        };

        let col_idx = data.schema().fields().iter()
            .position(|f| f.name() == &arrow_name);

        if let Some(idx) = col_idx {
            let min_max = data.column_min_max(idx);
            let percentiles = data.column_percentiles(idx, 0.05, 0.95);
            match (min_max, percentiles) {
                (Some((min, max)), Some((p5, p95))) =>
                    serde_json::json!({ "min": min, "max": max, "p5": p5, "p95": p95 }).to_string(),
                (Some((min, max)), None) =>
                    serde_json::json!({ "min": min, "max": max, "p5": min, "p95": max }).to_string(),
                _ => "null".to_string(),
            }
        } else {
            "null".to_string()
        }
    }

    pub fn get_scroll_metrics(&self) -> String {
        let total_header_height = self.state.header_height + self.state.group_header_height;
        let data_height = (self.state.height - total_header_height).max(0.0);
        let visible_rows = if self.state.row_height > 0.0 {
            data_height / self.state.row_height
        } else {
            0.0
        };

        let total_col_width: f64 = self.state.mapped_columns.iter().map(|c| c.width).sum();

        serde_json::json!({
            "total_rows": self.state.rows,
            "visible_rows": visible_rows,
            "cell_y_offset": self.state.cell_y_offset,
            "row_height": self.state.row_height,
            "total_col_width": total_col_width,
            "canvas_width": self.state.width,
            "canvas_height": self.state.height,
            "total_header_height": total_header_height,
            "translate_x": self.state.translate_x,
        }).to_string()
    }

    /// Returns "resize", "col-drag", "drag", or "none".
    pub fn on_mouse_down(&mut self, x: f64, y: f64) -> String {
        self.state.recompute_layout();

        let effective = self.state.effective_columns();
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let tx = self.state.translate_x;

        // Check resize border first (uses precomputed layout)
        if let Some(col_idx) = layout::hit_test_resize_border(
            x, y, &self.state.col_layout,
        ) {
            let col_width = self.state.col_layout.entry_by_source(col_idx)
                .map(|e| e.width)
                .unwrap_or(FALLBACK_COLUMN_WIDTH);
            self.state.resize_state = Some(ResizeState {
                column_display_index: col_idx,
                start_x: x,
                start_width: col_width,
                current_x: x,
            });
            return "resize".to_string();
        }

        // Check menu button (within leaf header row) — before col-drag check
        if let Some(col_idx) = layout::hit_test_menu_button(x, y, &self.state.col_layout) {
            self.state.last_menu_col = Some(col_idx);
            self.state.last_span_menu = None;
            return "header-menu".to_string();
        }

        // Check parent span menu buttons (non-leaf header rows, grouped mode only)
        if let Some(span) = layout::hit_test_span_menu_button(x, y, &self.state.col_layout) {
            self.state.last_span_menu = Some((span.level_idx, span.first_leaf, span.last_leaf));
            self.state.last_menu_col = None;
            return "span-menu".to_string();
        }

        // Check sort triangles — must come before col-drag so triangle clicks
        // don't start a column drag (they're handled by the click event via on_click).
        if layout::hit_test_sort_triangle(x, y, &self.state.col_layout).is_some() {
            return "none".to_string();
        }

        // Check header expand/collapse-all icon (group-key columns only, in leaf header row).
        if !self.state.group_key_display_cols.is_empty() {
            let leaf_y = self.state.col_layout.leaf_y;
            let leaf_h = self.state.col_layout.leaf_h;
            if y >= leaf_y && y <= leaf_y + leaf_h {
                let icon_size = crate::render::cells::EXPAND_ICON_SIZE;
                let icon_pad = crate::render::cells::EXPAND_ICON_PAD;
                let right_reserved = layout::header_right_reserved_width();
                for &gk_col in &self.state.group_key_display_cols {
                    if let Some(entry) = self.state.col_layout.entry_by_source(gk_col) {
                        // Icon is right-aligned within the virtual cell [entry.draw_x, entry.draw_x + entry.width - right_reserved]
                        let virtual_right = entry.draw_x + entry.width - right_reserved;
                        let icon_right = virtual_right - icon_pad;
                        let icon_left = icon_right - icon_size;
                        if x >= icon_left && x <= icon_right {
                            self.state.pending_expand_row = Some(gk_col);
                            return "header-expand-toggle".to_string();
                        }
                    }
                }
            }
        }

        // Check if mousedown is in the leaf header row (for column drag)
        let leaf_y = self.state.col_layout.leaf_y;
        let leaf_h = self.state.col_layout.leaf_h;
        if y >= leaf_y && y <= leaf_y + leaf_h {
            let col = hit_test::get_column_index_for_x(x, &effective, tx);
            if col >= 0 {
                let col_idx = col as usize;
                let title = effective
                    .iter()
                    .find(|c| c.source_index == col_idx)
                    .map(|c| c.title.clone())
                    .unwrap_or_default();
                self.state.col_drag = Some(ColDragState {
                    col_display_index: col_idx,
                    col_title: title,
                    start_x: x,
                    start_y: y,
                    mouse_x: x,
                    mouse_y: y,
                    prev_mouse_x: x,
                    has_activated: false,
                });
                return "col-drag".to_string();
            }
        }

        // Check expand/collapse icon — only in grouped mode with virtual rows
        if !self.state.group_key_display_cols.is_empty() {
            let h = self.state.height;
            let rh = self.state.row_height;
            let rows = self.state.rows;
            let cyo = self.state.cell_y_offset;
            let ty = self.state.translate_y;
            let ftr = self.state.freeze_trailing_rows;
            let has_groups = self.state.enable_groups;

            if let Some((col, row)) = hit_test::hit_test(
                x, y, &effective, h, h, has_groups, hh, ghh, rows, rh,
                self.state.cell_x_offset, cyo, tx, ty, ftr,
            ) {
                let col_idx = col as usize;
                let vrow_idx = row as usize;
                // Check if this column is any group-key column
                if let Some(depth) = self.state.group_key_display_cols.iter().position(|&c| c == col_idx) {
                    if row >= 0 {
                        if let Some(vrow) = self.state.virtual_rows.get(vrow_idx) {
                            // Only show expand icon on aggregate rows at matching depth
                            if vrow.is_aggregate() && vrow.depth() == depth {
                                if let Some(entry) = self.state.col_layout.entry_by_source(col_idx) {
                                    let icon_right = entry.draw_x + entry.width - 6.0;
                                    let icon_left = icon_right - 14.0;
                                    if x >= icon_left && x <= icon_right {
                                        self.state.pending_expand_row = Some(vrow_idx);
                                        return "expand-toggle".to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check if mousedown lands on a data cell → start drag selection
        let h = self.state.height;
        let rh = self.state.row_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
        let ty = self.state.translate_y;
        let ftr = self.state.freeze_trailing_rows;
        let has_groups = self.state.enable_groups;

        if let Some((col, row)) = hit_test::hit_test(
            x, y, &effective, h, h, has_groups, hh, ghh, rows, rh,
            self.state.cell_x_offset, cyo, tx, ty, ftr,
        ) {
            self.state.drag_start = Some(types::Item::new(col, row));
            return "drag".to_string();
        }

        "none".to_string()
    }

    pub fn get_last_expand_row(&self) -> i32 {
        self.state.pending_expand_row.map(|r| r as i32).unwrap_or(-1)
    }

    /// Expand or collapse all rows for the group-key column at the given display index.
    /// Expand-all is instant: marks all rows as expanded without fetching sub-data.
    /// `fetch_visible_expand_rows` then fills in data lazily as rows scroll into view.
    pub async fn toggle_header_expand(&mut self, display_col: usize) -> Result<(), JsValue> {
        let group_key_display_cols = self.state.group_key_display_cols.clone();
        let depth = match group_key_display_cols.iter().position(|&c| c == display_col) {
            Some(d) => d,
            None => return Ok(()),
        };

        if depth != 0 { return Ok(()); }

        let n_top = self.state.grouped_data.as_ref().map(|d| d.num_rows()).unwrap_or(0);
        let n_expanded_depth0 = self.state.expanded_keys.iter().filter(|k| k.len() == 1).count();
        let all_expanded = n_top > 0 && n_expanded_depth0 >= n_top;

        if all_expanded {
            self.state.expanded_keys.clear();
            self.state.expand_caches.clear();
            self.state.lazy_combined_data = None;
            self.state.rebuild_virtual_rows();
            return Ok(());
        }

        // Expand all — lazily. For multi-key case, pre-compute the combined group_by once
        // (one DataFusion query) and cache it for in-memory partitioning on demand.
        let group_keys = self.state.group_by_state.group_keys.clone();
        let aggregations = self.state.group_by_state.aggregations.clone();
        let n_keys = group_keys.len();

        if n_keys > 1 && self.state.lazy_combined_data.is_none() {
            let combined = if let Some(ref data) = self.state.arrow_data {
                data.group_by(&group_keys, &aggregations).await.ok()
            } else { None };
            self.state.lazy_combined_data = combined;
        }

        // Mark all top-level rows as expanded (no cache data yet → Pending rows).
        let first_result_name = group_keys[0].result_name();
        if let Some(grouped_data) = &self.state.grouped_data {
            let col_idx = grouped_data.schema().fields().iter()
                .position(|f| f.name().as_str() == first_result_name.as_str())
                .unwrap_or(0);
            let n = grouped_data.num_rows();
            for row in 0..n {
                let val = grouped_data.get_cell_raw_text(col_idx, row);
                let ck: types::ExpandCacheKey = vec![(first_result_name.clone(), val)];
                self.state.expanded_keys.insert(ck);
                // Intentionally do NOT insert into expand_caches → will be Pending
            }
        }
        self.state.rebuild_virtual_rows();
        Ok(())
    }

    /// Fetch sub-level data for any Pending rows currently visible on screen.
    /// Returns true if any data was fetched (caller should re-render).
    pub async fn fetch_visible_expand_rows(&mut self) -> Result<bool, JsValue> {
        if self.state.virtual_rows.is_empty() { return Ok(false); }

        let total_header_height = self.state.header_height + self.state.group_header_height;
        let data_height = (self.state.height - total_header_height).max(0.0);
        let visible_count = if self.state.row_height > 0.0 {
            (data_height / self.state.row_height).ceil() as usize + 2  // +2 buffer
        } else {
            30
        };
        let start = self.state.cell_y_offset;
        let end = (start + visible_count).min(self.state.virtual_rows.len());

        // Collect unique pending cache keys in the visible window.
        let mut seen = std::collections::HashSet::new();
        let pending_keys: Vec<types::ExpandCacheKey> = self.state.virtual_rows[start..end]
            .iter()
            .filter_map(|vrow| {
                if let types::VirtualRowRef::Pending { cache_key, .. } = vrow {
                    if seen.insert(cache_key.clone()) { Some(cache_key.clone()) } else { None }
                } else {
                    None
                }
            })
            .collect();

        if pending_keys.is_empty() { return Ok(false); }

        let group_keys = self.state.group_by_state.group_keys.clone();
        let aggregations = self.state.group_by_state.aggregations.clone();
        let n_keys = group_keys.len();

        for cache_key in &pending_keys {
            if self.state.expand_caches.contains_key(cache_key) { continue; }

            let is_leaf_level = cache_key.len() >= n_keys;

            let result = if n_keys > 1 && !is_leaf_level {
                // Use in-memory partition from lazy_combined_data (set during toggle_header_expand).
                if let Some(ref combined) = self.state.lazy_combined_data {
                    let first_result_name = group_keys[0].result_name();
                    let col_idx = combined.schema().fields().iter()
                        .position(|f| f.name().as_str() == first_result_name.as_str())
                        .unwrap_or(0);
                    let key_val = &cache_key[0].1;
                    Some(combined.rows_matching_column_value(col_idx, key_val))
                } else {
                    // Fallback: run filter_raw + group_by for this single visible row.
                    let filters: Vec<(types::DateGroupKey, String)> = cache_key.iter()
                        .filter_map(|(col_name, v)| {
                            group_keys.iter().find(|k| k.result_name() == *col_name)
                                .map(|k| (k.clone(), v.clone()))
                        }).collect();
                    if let Some(ref data) = self.state.arrow_data {
                        if let Ok(filtered) = data.filter_raw(&filters).await {
                            filtered.group_by(&group_keys[1..], &aggregations).await.ok()
                        } else { None }
                    } else { None }
                }
            } else {
                // Leaf level: filter_raw
                let filters: Vec<(types::DateGroupKey, String)> = cache_key.iter()
                    .filter_map(|(col_name, v)| {
                        group_keys.iter().find(|k| k.result_name() == *col_name)
                            .map(|k| (k.clone(), v.clone()))
                    }).collect();
                if let Some(ref data) = self.state.arrow_data {
                    data.filter_raw(&filters).await.ok()
                } else { None }
            };

            if let Some(data) = result {
                self.state.expand_caches.insert(cache_key.clone(), data);
            }
        }

        self.state.rebuild_virtual_rows();
        Ok(true)
    }

    pub async fn toggle_row_expand(&mut self, virtual_row: usize) -> Result<(), JsValue> {
        let depth = self.state.virtual_rows.get(virtual_row)
            .map(|v| v.depth())
            .unwrap_or(0);
        self.state.toggle_expand(virtual_row, depth).await
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Update drag selection while mouse is held. Call from mousemove when dragging.
    pub fn on_drag_update(&mut self, x: f64, y: f64) {
        let anchor = match self.state.drag_start {
            Some(a) => a,
            None => return,
        };

        let effective = self.state.effective_columns();
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let h = self.state.height;
        let rh = self.state.row_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
        let ty = self.state.translate_y;
        let tx = self.state.translate_x;
        let ftr = self.state.freeze_trailing_rows;
        let has_groups = self.state.enable_groups;

        // Clamp x/y to canvas bounds so dragging outside still extends selection
        let cx = x.clamp(0.0, self.state.width - 1.0);
        let cy = y.clamp(hh + ghh + 1.0, self.state.height - 1.0);

        let current_cell = hit_test::hit_test(
            cx, cy, &effective, h, h, has_groups, hh, ghh, rows, rh,
            self.state.cell_x_offset, cyo, tx, ty, ftr,
        );

        let (cur_col, cur_row) = current_cell.unwrap_or((anchor.col, anchor.row));

        let min_col = anchor.col.min(cur_col) as f64;
        let max_col = anchor.col.max(cur_col) as f64;
        let min_row = anchor.row.min(cur_row) as f64;
        let max_row = anchor.row.max(cur_row) as f64;

        self.state.selection = types::GridSelection {
            current: Some(types::CurrentSelection {
                cell: anchor,
                range: types::Rectangle::new(
                    min_col, min_row,
                    max_col - min_col + 1.0,
                    max_row - min_row + 1.0,
                ),
            }),
            columns: Vec::new(),
            rows: Vec::new(),
            ctrl_cells: Vec::new(),
        };
    }

    /// Commit drag selection on mouse release.
    /// `moved` — true if the mouse actually moved beyond threshold (drag),
    ///           false if this was a click-without-drag.
    pub fn on_drag_end(&mut self, x: f64, y: f64, moved: bool) {
        if moved {
            self.on_drag_update(x, y);
            self.state.drag_start = None;
            return;
        }

        // Click without drag — apply single-cell selection logic
        let anchor = self.state.drag_start.take();

        let effective = self.state.effective_columns();
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let h = self.state.height;
        let rh = self.state.row_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
        let ty = self.state.translate_y;
        let tx = self.state.translate_x;
        let ftr = self.state.freeze_trailing_rows;
        let has_groups = self.state.enable_groups;

        let hit = hit_test::hit_test(
            x, y, &effective, h, h, has_groups, hh, ghh, rows, rh,
            self.state.cell_x_offset, cyo, tx, ty, ftr,
        );

        // Multi-selection active + plain click → clear
        if self.state.selection.is_multi() {
            self.state.selection = types::GridSelection::default();
            return;
        }

        match hit {
            None => {
                // Click outside any cell — clear selection
                self.state.selection = types::GridSelection::default();
            }
            Some((col, row)) => {
                let already_selected = self.state.selection.current.as_ref()
                    .map_or(false, |cur| {
                        cur.cell.col == col && cur.cell.row == row
                    });
                if already_selected {
                    // Same cell clicked again — deselect
                    self.state.selection = types::GridSelection::default();
                } else {
                    self.state.selection = types::GridSelection::single(col, row);
                }
            }
        }
    }

    pub fn on_col_drag_end(&mut self) {
        self.state.col_drag = None;
    }

    pub fn get_drag_col_name(&self) -> String {
        self.state.col_drag
            .as_ref()
            .map(|d| d.col_title.clone())
            .unwrap_or_default()
    }

    pub fn is_col_drag_active(&self) -> bool {
        self.state.col_drag.as_ref().map_or(false, |d| d.has_activated)
    }

    pub fn get_drag_mouse_x(&self) -> f64 {
        self.state.col_drag.as_ref().map_or(0.0, |d| d.mouse_x)
    }

    pub fn get_drag_mouse_y(&self) -> f64 {
        self.state.col_drag.as_ref().map_or(0.0, |d| d.mouse_y)
    }

    pub fn on_mouse_up(&mut self, x: f64, y: f64) {
        self.state.drag_start = None;
        if let Some(rs) = self.state.resize_state.take() {
            let new_width = (rs.start_width + (x - rs.start_x)).max(MIN_COLUMN_WIDTH);
            if let Some(col) = self
                .state
                .mapped_columns
                .iter_mut()
                .find(|c| c.source_index == rs.column_display_index)
            {
                col.width = new_width;
            }
            if let Some(col) = self
                .state
                .columns
                .get_mut(rs.column_display_index)
            {
                col.width = new_width;
            }
            if let Some(ref mut resolved) = self.state.resolved_columns {
                if let Some(leaf) = resolved
                    .leaves
                    .iter_mut()
                    .find(|l| l.display_index == rs.column_display_index)
                {
                    leaf.width = new_width;
                }
            }
        }
    }

    /// Handle scroll wheel. Adjusts cell_y_offset and translate.
    pub fn on_scroll(&mut self, delta_x: f64, delta_y: f64) {
        let rh = self.state.row_height;
        let max_rows = self.state.rows.saturating_sub(self.state.freeze_trailing_rows);

        if self.state.smooth_scroll_y {
            self.state.translate_y -= delta_y;
            while self.state.translate_y <= -rh && self.state.cell_y_offset < max_rows {
                self.state.translate_y += rh;
                self.state.cell_y_offset += 1;
            }
            while self.state.translate_y >= rh && self.state.cell_y_offset > 0 {
                self.state.translate_y -= rh;
                self.state.cell_y_offset -= 1;
            }
            self.state.translate_y = self.state.translate_y.clamp(-rh, rh);
            if self.state.cell_y_offset == 0 {
                self.state.translate_y = self.state.translate_y.min(0.0);
            }
            if self.state.cell_y_offset >= max_rows {
                self.state.translate_y = self.state.translate_y.max(0.0);
            }
        } else {
            if delta_y > 0.0 && self.state.cell_y_offset < max_rows {
                self.state.cell_y_offset += 1;
            } else if delta_y < 0.0 && self.state.cell_y_offset > 0 {
                self.state.cell_y_offset -= 1;
            }
            self.state.translate_y = 0.0;
        }

        if delta_x != 0.0 {
            let total_col_width: f64 = self.state.columns.iter().map(|c| c.width).sum();
            self.state.translate_x -= delta_x;
            self.state.translate_x = self.state.translate_x.clamp(
                -(total_col_width - self.state.width).max(0.0),
                0.0,
            );
        }
    }

    /// Handle keyboard navigation.
    pub fn on_key_down(&mut self, key: &str, shift: bool, ctrl: bool) {
        let max_rows = self.state.rows as i32;
        let max_cols = self.state.columns.len() as i32;

        if let Some(ref current) = self.state.selection.current {
            let mut col = current.cell.col;
            let mut row = current.cell.row;

            match key {
                "ArrowUp" => {
                    if row > -1 { row -= 1; }
                }
                "ArrowDown" => {
                    if row < max_rows - 1 { row += 1; }
                }
                "ArrowLeft" => {
                    if col > 0 { col -= 1; }
                }
                "ArrowRight" => {
                    if col < max_cols - 1 { col += 1; }
                }
                "Home" => {
                    col = 0;
                    if ctrl { row = -1; }
                }
                "End" => {
                    col = max_cols - 1;
                    if ctrl { row = max_rows - 1; }
                }
                "PageUp" => {
                    row = (row - PAGE_SCROLL_ROWS).max(-1);
                }
                "PageDown" => {
                    row = (row + PAGE_SCROLL_ROWS).min(max_rows - 1);
                }
                _ => return,
            }

            self.state.selection = types::GridSelection::single(col, row);
            self.scroll_to_cell(col, row);
        }
    }

    /// Get the current selection as JSON.
    pub fn get_selection(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.state.selection).unwrap_or(JsValue::NULL)
    }

    /// Get bounds of a cell as JSON { x, y, width, height }.
    pub fn get_bounds(&self, col: i32, row: i32) -> JsValue {
        let effective = self.state.effective_columns();
        if let Some((x, y, w, h)) = hit_test::get_cell_bounds(
            col as usize,
            row,
            &effective,
            self.state.height,
            self.state.header_height,
            self.state.group_header_height,
            self.state.rows,
            self.state.row_height,
            self.state.cell_y_offset,
            self.state.translate_x,
            self.state.translate_y,
            self.state.freeze_trailing_rows,
        ) {
            serde_wasm_bindgen::to_value(&types::Rectangle::new(x, y, w, h))
                .unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    }
}

impl DataGrid {
    fn scroll_to_cell(&mut self, col: i32, row: i32) {
        if row >= 0 {
            let row_idx = row as usize;
            if row_idx < self.state.cell_y_offset {
                self.state.cell_y_offset = row_idx;
                self.state.translate_y = 0.0;
            } else {
                let total_header = self.state.header_height + self.state.group_header_height;
                let available_height = self.state.height - total_header;
                let visible_rows = (available_height / self.state.row_height).floor() as usize;
                if row_idx >= self.state.cell_y_offset + visible_rows {
                    self.state.cell_y_offset = row_idx.saturating_sub(visible_rows) + 1;
                    self.state.translate_y = 0.0;
                }
            }
        }
    }

    fn restore_original_data(&mut self) -> Result<(), JsValue> {
        if let Some(ref bytes) = self.state.original_ipc_bytes {
            let data = arrow_data::ArrowDataSource::from_ipc_stream(bytes)
                .map_err(|e| JsValue::from_str(&e))?;
            self.state.rows = data.num_rows();
            self.state.arrow_data = Some(data);
        }
        Ok(())
    }
}
