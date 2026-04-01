pub mod canvas;
pub mod color;
pub mod grid;
pub mod hit_test;
pub mod render;
pub mod theme;
pub mod types;
pub mod walk;

use canvas::CanvasCtx;
use grid::GridState;
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

    /// Set header height in pixels.
    pub fn set_header_height(&mut self, h: f64) {
        self.state.header_height = h;
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
    /// { current?: { cell: [col, row], range: { x, y, width, height } }, columns: number[], rows: number[] }
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

    /// Handle mouse click. Selects the cell at (x, y).
    pub fn on_click(&mut self, x: f64, y: f64, shift: bool, ctrl: bool) {
        let effective = self.state.effective_columns();
        let h = self.state.height;
        let rh = self.state.row_height;
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
        let ty = self.state.translate_y;
        let tx = self.state.translate_x;
        let ftr = self.state.freeze_trailing_rows;
        let has_groups = self.state.enable_groups;

        if let Some((col, row)) = hit_test::hit_test(
            x, y, &effective, h, h, has_groups, hh, ghh, rows, rh,
            self.state.cell_x_offset, cyo, tx, ty, ftr,
        ) {
            self.state.selection = types::GridSelection {
                current: Some(types::CurrentSelection {
                    cell: types::Item::new(col, row),
                    range: types::Rectangle::new(col as f64, row as f64, 1.0, 1.0),
                }),
                columns: Vec::new(),
                rows: Vec::new(),
            };
        }
    }

    /// Handle mouse move. Returns hovered cell as JSON, or null.
    pub fn on_mouse_move(&mut self, x: f64, y: f64) -> JsValue {
        let effective = self.state.effective_columns();
        let h = self.state.height;
        let rh = self.state.row_height;
        let hh = self.state.header_height;
        let ghh = self.state.group_header_height;
        let rows = self.state.rows;
        let cyo = self.state.cell_y_offset;
        let ty = self.state.translate_y;
        let tx = self.state.translate_x;
        let ftr = self.state.freeze_trailing_rows;
        let has_groups = self.state.enable_groups;

        if let Some((col, row)) = hit_test::hit_test(
            x, y, &effective, h, h, has_groups, hh, ghh, rows, rh,
            self.state.cell_x_offset, cyo, tx, ty, ftr,
        ) {
            serde_wasm_bindgen::to_value(&[col, row]).unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    }

    /// Handle scroll wheel. Adjusts cell_y_offset and translate.
    pub fn on_scroll(&mut self, delta_x: f64, delta_y: f64) {
        let rh = self.state.row_height;
        let max_rows = self.state.rows.saturating_sub(self.state.freeze_trailing_rows);

        if self.state.smooth_scroll_y {
            self.state.translate_y -= delta_y;
            // Snap to cell boundaries
            while self.state.translate_y <= -rh && self.state.cell_y_offset < max_rows {
                self.state.translate_y += rh;
                self.state.cell_y_offset += 1;
            }
            while self.state.translate_y >= rh && self.state.cell_y_offset > 0 {
                self.state.translate_y -= rh;
                self.state.cell_y_offset -= 1;
            }
            // Clamp
            self.state.translate_y = self.state.translate_y.clamp(-rh, rh);
            if self.state.cell_y_offset == 0 {
                self.state.translate_y = self.state.translate_y.min(0.0);
            }
            if self.state.cell_y_offset >= max_rows {
                self.state.translate_y = self.state.translate_y.max(0.0);
            }
        } else {
            // Discrete scrolling
            if delta_y > 0.0 && self.state.cell_y_offset < max_rows {
                self.state.cell_y_offset += 1;
            } else if delta_y < 0.0 && self.state.cell_y_offset > 0 {
                self.state.cell_y_offset -= 1;
            }
            self.state.translate_y = 0.0;
        }

        // Horizontal scroll
        let total_col_width: f64 = self.state.columns.iter().map(|c| c.width).sum();
        if self.state.smooth_scroll_x {
            self.state.translate_x -= delta_x;
            // Clamp horizontal
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
                    if row > -1 {
                        row -= 1;
                    }
                }
                "ArrowDown" => {
                    if row < max_rows - 1 {
                        row += 1;
                    }
                }
                "ArrowLeft" => {
                    if col > 0 {
                        col -= 1;
                    }
                }
                "ArrowRight" => {
                    if col < max_cols - 1 {
                        col += 1;
                    }
                }
                "Home" => {
                    col = 0;
                    if ctrl {
                        row = -1;
                    }
                }
                "End" => {
                    col = max_cols - 1;
                    if ctrl {
                        row = max_rows - 1;
                    }
                }
                "PageUp" => {
                    row = (row - 20).max(-1);
                }
                "PageDown" => {
                    row = (row + 20).min(max_rows - 1);
                }
                _ => return,
            }

            self.state.selection = types::GridSelection {
                current: Some(types::CurrentSelection {
                    cell: types::Item::new(col, row),
                    range: types::Rectangle::new(col as f64, row as f64, 1.0, 1.0),
                }),
                columns: Vec::new(),
                rows: Vec::new(),
            };

            // Auto-scroll to keep selected cell visible
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
        // Scroll vertically to show the row
        if row >= 0 {
            let row_idx = row as usize;
            if row_idx < self.state.cell_y_offset {
                self.state.cell_y_offset = row_idx;
                self.state.translate_y = 0.0;
            } else {
                // Check if row is visible below
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
}
