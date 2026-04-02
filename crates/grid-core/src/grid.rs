use crate::arrow_data::ArrowDataSource;
use crate::canvas::CanvasCtx;
use crate::columns::{normalize_columns, resolve_columns, ResolvedColumns};
use crate::render::header::GroupDetails;
use crate::theme::Theme;
use crate::types::{ColDragState, ColumnInput, GridCell, GridColumn, GridSelection, Item, ResizeState, SortState};
use crate::walk::MappedColumn;

pub struct GridState {
    pub canvas: Option<CanvasCtx>,
    pub width: f64,
    pub height: f64,
    pub columns: Vec<GridColumn>,
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

    pub arrow_data: Option<ArrowDataSource>,
    pub get_cell_content_cb: Option<js_sys::Function>,

    pub sort_state: SortState,
    pub original_ipc_bytes: Option<Vec<u8>>,

    pub column_input: Option<Vec<ColumnInput>>,
    pub column_overrides: Option<Vec<ColumnInput>>,
    pub resolved_columns: Option<ResolvedColumns>,

    pub resize_state: Option<ResizeState>,
    pub resize_hover_col: Option<usize>,

    pub drag_start: Option<Item>,

    pub col_drag: Option<ColDragState>,
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
            arrow_data: None,
            get_cell_content_cb: None,
            sort_state: SortState::default(),
            original_ipc_bytes: None,
            column_input: None,
            column_overrides: None,
            resolved_columns: None,
            resize_state: None,
            resize_hover_col: None,
            drag_start: None,
            col_drag: None,
        }
    }

    pub fn set_data_from_ipc(&mut self, bytes: &[u8]) -> Result<(), String> {
        let data = ArrowDataSource::from_ipc_stream(bytes)?;
        self.rows = data.num_rows();
        self.arrow_data = Some(data);
        self.original_ipc_bytes = Some(bytes.to_vec());
        self.sort_state = SortState::default();
        Ok(())
    }

    pub fn save_original_bytes(&mut self) {
        if let Some(ref data) = self.arrow_data {
            if let Ok(bytes) = data.to_ipc_stream() {
                self.original_ipc_bytes = Some(bytes);
            }
        }
    }

    pub fn auto_configure_from_data(&mut self, default_col_width: f64) {
        let (num_rows, data_col_names, default_grid_cols) = match self.arrow_data {
            Some(ref data) => {
                let names: Vec<String> = (0..data.num_columns())
                    .map(|i| data.column_name(i).to_string())
                    .collect();
                let cols = data.to_grid_columns(default_col_width);
                (data.num_rows(), names, cols)
            }
            None => return,
        };

        self.rows = num_rows;

        let has_user_columns = self.column_input.is_some() || self.column_overrides.is_some();

        if has_user_columns {
            if let Err(e) = self.configure_columns(&data_col_names, default_col_width) {
                web_sys::console::error_1(
                    &wasm_bindgen::JsValue::from_str(&format!("Column config error: {}", e)),
                );
                self.columns = default_grid_cols;
                self.resolved_columns = None;
                self.enable_groups = false;
                self.group_header_height = 0.0;
                self.remap_columns();
            }
        } else {
            self.columns = default_grid_cols;
            self.resolved_columns = None;
            self.enable_groups = false;
            self.group_header_height = 0.0;
            self.remap_columns();
        }
    }

    pub fn configure_columns(
        &mut self,
        data_col_names: &[String],
        default_col_width: f64,
    ) -> Result<(), String> {
        let normalized = normalize_columns(
            self.column_input.as_deref(),
            self.column_overrides.as_deref(),
            data_col_names,
        )?;

        let resolved = resolve_columns(&normalized, data_col_names, default_col_width, 1.0)?;

        let grid_columns: Vec<GridColumn> = resolved
            .leaves
            .iter()
            .map(|leaf| GridColumn {
                title: leaf.display_name.clone(),
                width: leaf.width,
                group: None,
                icon: None,
                id: Some(leaf.arrow_name.clone()),
            })
            .collect();

        self.columns = grid_columns;

        if resolved.max_depth > 1 {
            self.enable_groups = true;
            let level_height = self.header_height / resolved.max_depth as f64;
            self.group_header_height =
                level_height * (resolved.max_depth - 1) as f64;
        } else {
            self.enable_groups = false;
            self.group_header_height = 0.0;
        }

        self.resolved_columns = Some(resolved);
        self.remap_columns();
        Ok(())
    }

    pub fn remap_columns(&mut self) {
        if let Some(ref resolved) = self.resolved_columns {
            self.mapped_columns = resolved
                .leaves
                .iter()
                .map(|leaf| MappedColumn {
                    title: leaf.display_name.clone(),
                    width: leaf.width,
                    group: None,
                    icon: None,
                    source_index: leaf.display_index,
                    arrow_index: leaf.arrow_index,
                    sticky: leaf.display_index < self.freeze_columns,
                    is_resizable: leaf.is_resizable,
                })
                .collect();
        } else {
            self.mapped_columns = self
                .columns
                .iter()
                .enumerate()
                .map(|(i, c)| MappedColumn::from_column(c, i, i < self.freeze_columns))
                .collect();
        }
    }

    /// Remap selection row indices after a sort using stored row IDs.
    /// Call this after replacing `self.arrow_data` with sorted data.
    pub fn remap_selection_after_sort(&mut self) {
        let data = match &self.arrow_data {
            Some(d) => d,
            None => return,
        };

        // Remap current anchor + range
        if let Some(ref mut cur) = self.selection.current {
            let old_row = cur.cell.row;
            let old_range_row = cur.range.y as i32;
            let old_range_row2 = (cur.range.y + cur.range.height - 1.0) as i32;

            // Anchor
            if let Some(row_id) = data.get_row_id(old_row as usize) {
                if let Some(new_row) = data.find_row_by_id(row_id) {
                    cur.cell.row = new_row as i32;
                }
            } else if let Some(new_row) = data.find_row_by_id(old_row as u64) {
                cur.cell.row = new_row as i32;
            }

            // Range — remap both corners
            let new_row1 = data.find_row_by_id(old_range_row as u64)
                .map(|r| r as i32)
                .unwrap_or(cur.cell.row);
            let new_row2 = data.find_row_by_id(old_range_row2 as u64)
                .map(|r| r as i32)
                .unwrap_or(cur.cell.row);
            let min_row = new_row1.min(new_row2);
            let max_row = new_row1.max(new_row2);
            cur.range.y = min_row as f64;
            cur.range.height = (max_row - min_row + 1) as f64;
        }

        // Remap ctrl_cells
        let new_ctrl: Vec<Item> = self.selection.ctrl_cells
            .iter()
            .filter_map(|item| {
                data.find_row_by_id(item.row as u64)
                    .map(|new_row| Item::new(item.col, new_row as i32))
            })
            .collect();
        self.selection.ctrl_cells = new_ctrl;
    }

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

        let sticky_width: f64 = sticky.iter().map(|c| c.width).sum();
        let available = self.width - sticky_width;

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

    pub fn get_cell_content(&self, col: i32, row: i32) -> GridCell {
        if col < 0 || row < 0 {
            return GridCell::loading();
        }

        let arrow_col = self.display_to_arrow_index(col as usize);

        if let Some(ref data) = self.arrow_data {
            return data.get_cell(arrow_col, row as usize);
        }

        if let Some(ref cb) = self.get_cell_content_cb {
            let this = wasm_bindgen::JsValue::null();
            let col_val = wasm_bindgen::JsValue::from_f64(col as f64);
            let row_val = wasm_bindgen::JsValue::from_f64(row as f64);
            match cb.call2(&this, &col_val, &row_val) {
                Ok(val) => serde_wasm_bindgen::from_value::<GridCell>(val)
                    .unwrap_or_else(|_| GridCell::loading()),
                Err(_) => GridCell::loading(),
            }
        } else {
            GridCell::loading()
        }
    }

    pub fn display_to_arrow_index(&self, display_index: usize) -> usize {
        if let Some(ref resolved) = self.resolved_columns {
            resolved
                .leaves
                .get(display_index)
                .map(|l| l.arrow_index)
                .unwrap_or(display_index)
        } else {
            display_index
        }
    }

    pub fn column_x_position(&self, display_index: usize) -> f64 {
        let mut x = 0.0;
        for c in &self.mapped_columns {
            if c.source_index == display_index {
                return x;
            }
            x += c.width;
        }
        x
    }

    pub fn swap_columns(&mut self, a: usize, b: usize) {
        if a == b { return; }
        if let Some(ref mut resolved) = self.resolved_columns {
            resolved.swap_leaves(a, b);
        }
        if a < self.columns.len() && b < self.columns.len() {
            self.columns.swap(a, b);
        }
        self.remap_columns();
    }

    pub fn get_group_details(&self, _name: &str) -> GroupDetails {
        GroupDetails::default()
    }

    pub fn vertical_border(&self, _col: usize) -> bool {
        true
    }

    pub fn render(&mut self) {
        let effective = self.effective_columns();
        let mapped = self.mapped_columns.clone();
        let theme = self.theme.clone();
        let selection = self.selection.clone();
        let sort_state = self.sort_state.clone();
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
        let resolved = self.resolved_columns.clone();
        let resize_state = self.resize_state.clone();
        let col_drag = self.col_drag.clone();

        let arrow_ref = self.arrow_data.as_ref();
        let cb_ref = self.get_cell_content_cb.as_ref();
        let resolved_ref = &resolved;
        let get_cell_content = |col: i32, row: i32| -> GridCell {
            if col < 0 || row < 0 {
                return GridCell::loading();
            }
            let arrow_col = if let Some(ref r) = resolved_ref {
                r.leaves
                    .get(col as usize)
                    .map(|l| l.arrow_index)
                    .unwrap_or(col as usize)
            } else {
                col as usize
            };
            if let Some(data) = arrow_ref {
                return data.get_cell(arrow_col, row as usize);
            }
            if let Some(cb) = cb_ref {
                let this = wasm_bindgen::JsValue::null();
                let col_val = wasm_bindgen::JsValue::from_f64(col as f64);
                let row_val = wasm_bindgen::JsValue::from_f64(row as f64);
                match cb.call2(&this, &col_val, &row_val) {
                    Ok(val) => serde_wasm_bindgen::from_value::<GridCell>(val)
                        .unwrap_or_else(|_| GridCell::loading()),
                    Err(_) => GridCell::loading(),
                }
            } else {
                GridCell::loading()
            }
        };

        let get_group_details = |_name: &str| GroupDetails::default();
        let vertical_border = |_col: usize| true;

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
            &sort_state,
            &theme,
            is_focused,
            true,
            &get_cell_content,
            &get_group_details,
            &vertical_border,
            resolved.as_ref(),
            resize_state.as_ref(),
            col_drag.as_ref(),
        );
    }
}
