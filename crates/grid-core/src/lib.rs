pub mod arrow_data;
pub mod canvas;
pub mod color;
pub mod columns;
pub mod grid;
pub mod hit_test;
pub mod number_format;
pub mod render;
pub mod theme;
pub mod types;
pub mod walk;

use canvas::CanvasCtx;
use grid::GridState;
use types::{ColDragState, ColumnInput, ResizeState, SortDirection, SortState};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct DataGrid {
    state: GridState,
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
        state.canvas = Some(CanvasCtx::new(ctx));
        state.width = width;
        state.height = height;

        Ok(DataGrid { state })
    }

    /// Set the grid dimensions (e.g. after a resize).
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
        self.state.auto_configure_from_data(150.0);
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
        self.state.auto_configure_from_data(150.0);
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
                self.state.auto_configure_from_data(150.0);
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
        self.state.auto_configure_from_data(150.0);
        Ok(())
    }

    pub fn set_column_overrides(&mut self, json_str: &str) -> Result<(), JsValue> {
        let inputs: Vec<ColumnInput> = serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid column overrides JSON: {}", e)))?;
        self.state.column_overrides = Some(inputs);
        self.state.column_input = None;
        self.state.auto_configure_from_data(150.0);
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
            // Restore original data
            self.restore_original_data()?;
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
        let (col, ascending) = match &self.state.sort_state {
            SortState { column: Some(c), direction: Some(dir) } => {
                (*c, matches!(dir, SortDirection::Ascending))
            }
            _ => {
                self.restore_original_data()?;
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
        }

        Ok(())
    }

    /// Clear sort and restore original data order.
    pub fn clear_sort(&mut self) -> Result<(), JsValue> {
        self.state.sort_state = SortState::default();
        self.restore_original_data()?;
        self.state.remap_selection_after_sort();
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
        self.state.render();
    }

    // --- Event forwarding (called from JS) ---

    /// Handle click events.
    /// Plain cell selection is owned by on_drag_end (mouseup path).
    /// on_click handles: sort triangles, shift+click range extend, ctrl+click toggle.
    pub fn on_click(&mut self, x: f64, y: f64, shift: bool, ctrl: bool) {
        let effective = self.state.effective_columns();
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let tx = self.state.translate_x;

        // Sort triangle clicks anywhere in the total header area
        // (hit_test_sort_triangle checks the exact leaf row geometry internally)
        if y >= 0.0 && y <= hh + ghh {
            let col = hit_test::get_column_index_for_x(x, &effective, tx);
            if col >= 0 {
                if let Some(is_up) = render::header::hit_test_sort_triangle(
                    x, y, col as usize, &effective, hh, ghh, tx,
                    self.state.resolved_columns.as_ref(),
                ) {
                    let col_idx = col as usize;
                    let was_active = self.state.sort_state.column == Some(col_idx);
                    if is_up {
                        if was_active && self.state.sort_state.direction == Some(SortDirection::Ascending) {
                            self.state.sort_state = SortState::default();
                            let _ = self.restore_original_data();
                        } else {
                            self.state.sort_state = SortState {
                                column: Some(col_idx),
                                direction: Some(SortDirection::Ascending),
                            };
                        }
                    } else {
                        if was_active && self.state.sort_state.direction == Some(SortDirection::Descending) {
                            self.state.sort_state = SortState::default();
                            let _ = self.restore_original_data();
                        } else {
                            self.state.sort_state = SortState {
                                column: Some(col_idx),
                                direction: Some(SortDirection::Descending),
                            };
                        }
                    }
                    return;
                }
            }
            // Click in header but not on a triangle — do nothing
            return;
        }

        // No modifier keys and no multi-selection — plain cell clicks are handled
        // by on_drag_end already, so skip here to avoid double-handling.
        if !shift && !ctrl {
            return;
        }

        let h = self.state.height;
        let rh = self.state.row_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
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
                    if dx > 8.0 {
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

        let effective = self.state.effective_columns();
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let tx = self.state.translate_x;

        if let Some(ref mut rs) = self.state.resize_state {
            rs.current_x = x;
            return "col-resize".to_string();
        }

        if let Some(_col_idx) = render::header::hit_test_resize_border(
            x, y, &effective, hh, ghh, tx, 5.0,
            self.state.resolved_columns.as_ref(),
        ) {
            self.state.resize_hover_col = Some(_col_idx);
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

        if moving_right && drag_pos + 1 < col_positions.len() {
            let (neighbor_si, neighbor_x, neighbor_w) = col_positions[drag_pos + 1];
            let threshold = neighbor_x + neighbor_w * 0.1;
            if mouse_x > threshold {
                return Some((drag_col, neighbor_si));
            }
        }

        if !moving_right && drag_pos > 0 {
            let (neighbor_si, neighbor_x, neighbor_w) = col_positions[drag_pos - 1];
            let threshold = neighbor_x + neighbor_w * 0.9;
            if mouse_x < threshold {
                return Some((drag_col, neighbor_si));
            }
        }

        None
    }

    fn perform_col_swap(&mut self, a: usize, b: usize) {
        use crate::types::ColSlideAnimation;

        // Fast-forward any existing animation
        self.state.col_slide_anim = None;

        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let (leaf_y, _leaf_h) = render::header::leaf_row_geometry(
            hh, ghh, self.state.resolved_columns.as_ref(),
        );
        let canvas_h = self.state.height;
        let capture_y = leaf_y;
        let capture_h = canvas_h - leaf_y;

        // Find pixel positions of both columns before swap
        let tx = self.state.translate_x;
        let mut a_x = 0.0f64;
        let mut a_w = 0.0f64;
        let mut b_x = 0.0f64;
        let mut b_w = 0.0f64;
        {
            let mut x_acc = 0.0f64;
            for c in &self.state.mapped_columns {
                let draw_x = if c.sticky { x_acc } else { x_acc + tx };
                if c.source_index == a {
                    a_x = draw_x;
                    a_w = c.width;
                }
                if c.source_index == b {
                    b_x = draw_x;
                    b_w = c.width;
                }
                x_acc += c.width;
            }
        }

        // Capture pre-swap column strips
        let canvas_a = self.state.canvas.as_ref()
            .and_then(|ctx| ctx.capture_rect(a_x, capture_y, a_w, capture_h).ok());
        let canvas_b = self.state.canvas.as_ref()
            .and_then(|ctx| ctx.capture_rect(b_x, capture_y, b_w, capture_h).ok());

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
        self.state.render();

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
        self.state.col_slide_anim.is_some()
    }

    /// Returns "resize", "drag", or "none".
    pub fn on_mouse_down(&mut self, x: f64, y: f64) -> String {
        let effective = self.state.effective_columns();
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let tx = self.state.translate_x;

        // Check resize border first
        if let Some(col_idx) = render::header::hit_test_resize_border(
            x, y, &effective, hh, ghh, tx, 5.0,
            self.state.resolved_columns.as_ref(),
        ) {
            let col_width = effective
                .iter()
                .find(|c| c.source_index == col_idx)
                .map(|c| c.width)
                .unwrap_or(100.0);
            self.state.resize_state = Some(ResizeState {
                column_display_index: col_idx,
                start_x: x,
                start_width: col_width,
                current_x: x,
            });
            return "resize".to_string();
        }

        // Check if mousedown is in the leaf header row (for column drag)
        let total_hh = hh + ghh;
        let (leaf_y, leaf_h) = render::header::leaf_row_geometry(
            hh, ghh, self.state.resolved_columns.as_ref(),
        );
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
            let new_width = (rs.start_width + (x - rs.start_x)).max(30.0);
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

        let total_col_width: f64 = self.state.columns.iter().map(|c| c.width).sum();
        if self.state.smooth_scroll_x {
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
                    row = (row - 20).max(-1);
                }
                "PageDown" => {
                    row = (row + 20).min(max_rows - 1);
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
