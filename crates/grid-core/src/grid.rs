use crate::canvas::CanvasCtx;
use crate::render::header::GroupDetails;
use crate::theme::Theme;
use crate::types::{GridCell, GridSelection};
use crate::walk::MappedColumn;

/// The main grid state. Holds all configuration, scroll state, and selection.
pub struct GridState {
    pub canvas: Option<CanvasCtx>,
    pub width: f64,
    pub height: f64,
    pub columns: Vec<crate::types::GridColumn>,
    pub mapped_columns: Vec<MappedColumn>,
    pub rows: usize,
    pub cell_x_offset: usize,
    pub cell_y_offset: usize,
    pub translate_x: f64,
    pub translate_y: f64,
    pub header_height: f64,
    pub group_header_height: f64,
    pub freeze_columns: usize,
    pub freeze_trailing_rows: usize,
    pub row_height: f64,
    pub theme: Theme,
    pub selection: GridSelection,
    pub is_focused: bool,
    pub has_append_row: bool,
    pub enable_groups: bool,
    pub smooth_scroll_x: bool,
    pub smooth_scroll_y: bool,

    // JS callback for cell content
    pub get_cell_content_cb: Option<js_sys::Function>,
}

impl GridState {
    pub fn new() -> Self {
        Self {
            canvas: None,
            width: 0.0,
            height: 0.0,
            columns: Vec::new(),
            mapped_columns: Vec::new(),
            rows: 0,
            cell_x_offset: 0,
            cell_y_offset: 0,
            translate_x: 0.0,
            translate_y: 0.0,
            header_height: 36.0,
            group_header_height: 0.0,
            freeze_columns: 0,
            freeze_trailing_rows: 0,
            row_height: 34.0,
            theme: Theme::default_theme(),
            selection: GridSelection::default(),
            is_focused: true,
            has_append_row: false,
            enable_groups: false,
            smooth_scroll_x: false,
            smooth_scroll_y: false,
            get_cell_content_cb: None,
        }
    }

    /// Recompute mapped columns from the column list.
    pub fn remap_columns(&mut self) {
        self.mapped_columns = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| MappedColumn::from_column(c, i, i < self.freeze_columns))
            .collect();
    }

    /// Get the effective (visible) columns for the current scroll position.
    pub fn effective_columns(&self) -> Vec<MappedColumn> {
        let mut sticky: Vec<MappedColumn> = Vec::new();
        let mut scrolled: Vec<MappedColumn> = Vec::new();

        for c in &self.mapped_columns {
            if c.sticky {
                sticky.push(c.clone());
            } else if c.source_index >= self.cell_x_offset {
                scrolled.push(c.clone());
            }
        }

        // Calculate how much width we need
        let sticky_width: f64 = sticky.iter().map(|c| c.width).sum();
        let available = self.width - sticky_width;

        // Trim scrolled columns to fit
        let mut acc_width = 0.0f64;
        let mut end_idx = 0;
        for (i, c) in scrolled.iter().enumerate() {
            acc_width += c.width;
            end_idx = i + 1;
            if acc_width > available {
                break;
            }
        }
        scrolled.truncate(end_idx);

        sticky.extend(scrolled);
        sticky
    }

    /// Call the JS callback to get cell content.
    pub fn get_cell_content(&self, col: i32, row: i32) -> GridCell {
        if let Some(ref cb) = self.get_cell_content_cb {
            let this = wasm_bindgen::JsValue::null();
            let col_val = wasm_bindgen::JsValue::from_f64(col as f64);
            let row_val = wasm_bindgen::JsValue::from_f64(row as f64);

            match cb.call2(&this, &col_val, &row_val) {
                Ok(val) => {
                    match serde_wasm_bindgen::from_value::<GridCell>(val) {
                        Ok(cell) => cell,
                        Err(_) => GridCell::loading(),
                    }
                }
                Err(_) => GridCell::loading(),
            }
        } else {
            GridCell::loading()
        }
    }

    pub fn get_group_details(&self, _name: &str) -> GroupDetails {
        GroupDetails::default()
    }

    pub fn vertical_border(&self, _col: usize) -> bool {
        true
    }

    /// Render the grid to the canvas.
    pub fn render(&mut self) {
        // First, extract all the data we need from self into local variables
        // so the borrow checker doesn't complain about partial borrows.

        let effective = self.effective_columns();
        let mapped = self.mapped_columns.clone();
        let theme = self.theme.clone();
        let selection = self.selection.clone();
        let is_focused = self.is_focused;
        let rows = self.rows;
        let row_height = self.row_height;
        let header_height = self.header_height;
        let group_header_height = self.group_header_height;
        let enable_groups = self.enable_groups;
        let cell_y_offset = self.cell_y_offset;
        let translate_x = self.translate_x;
        let translate_y = self.translate_y;
        let freeze_columns = self.freeze_columns;
        let freeze_trailing_rows = self.freeze_trailing_rows;
        let has_append_row = self.has_append_row;
        let width = self.width;
        let height = self.height;

        // We need the callback as a reference. Take it out temporarily.
        let cb_ref = self.get_cell_content_cb.as_ref();
        let get_cell_content = |col: i32, row: i32| -> GridCell {
            if let Some(cb) = cb_ref {
                let this = wasm_bindgen::JsValue::null();
                let col_val = wasm_bindgen::JsValue::from_f64(col as f64);
                let row_val = wasm_bindgen::JsValue::from_f64(row as f64);
                match cb.call2(&this, &col_val, &row_val) {
                    Ok(val) => serde_wasm_bindgen::from_value::<GridCell>(val).unwrap_or_else(|_| GridCell::loading()),
                    Err(_) => GridCell::loading(),
                }
            } else {
                GridCell::loading()
            }
        };

        let get_group_details = |_name: &str| GroupDetails::default();
        let vertical_border = |_col: usize| true;

        // Now take the canvas as mutable
        let ctx = match &mut self.canvas {
            Some(c) => c,
            None => return,
        };

        crate::render::draw_grid(
            ctx,
            width,
            height,
            &effective,
            &mapped,
            rows,
            row_height,
            header_height,
            group_header_height,
            enable_groups,
            0,
            cell_y_offset,
            translate_x,
            translate_y,
            freeze_columns,
            freeze_trailing_rows,
            has_append_row,
            &selection,
            &theme,
            is_focused,
            true,
            &get_cell_content,
            &get_group_details,
            &vertical_border,
        );
    }
}
