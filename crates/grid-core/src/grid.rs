use crate::arrow_data::ArrowDataSource;
use crate::columns::{normalize_columns, resolve_columns, ResolvedColumns};
use crate::defaults::*;
use crate::render::header::GroupDetails;
use crate::theme::Theme;
use crate::layout::{self, ColumnLayout};
use crate::types::{AggregateFunction, AvailableAggFunctions, AvailableDateTruncations, ColDragState, ColSlideAnimation, ColumnInput, DateTruncationOptions, ExpandCacheKey, GridCell, GridColumn, GridSelection, GroupByState, Item, NumberFormat, ResizeState, SortState, VirtualRowRef};
use crate::walk::MappedColumn;

/// Describes how a grouped-result display column maps to source data for raw-row rendering.
#[derive(Debug, Clone)]
pub struct RawColMapping {
    /// Display index of this column.
    pub display_idx: usize,
    /// True if this is a group-key column (show blank for raw rows).
    pub is_group_key: bool,
    /// For value columns: the arrow_index in the original source schema.
    pub source_arrow_idx: Option<usize>,
    /// If this is the first sub-column of a multi-agg group, how many siblings follow
    /// (those siblings will return GridCell::Skip).
    pub merge_span: usize,
    /// True if this column should be skipped (it's a non-first sibling in a merge group).
    pub is_merge_skip: bool,
}

pub struct GridState {
    /// Canvas dimensions, kept in sync with the physical canvas by set_size().
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
    pub col_slide_anim: Option<ColSlideAnimation>,
    pub swap_animation_duration_ms: f64,
    pub col_layout: ColumnLayout,

    pub group_by_state: GroupByState,
    pub grouped_data: Option<ArrowDataSource>,
    pub original_column_input_snapshot: Option<Vec<ColumnInput>>,
    pub original_column_overrides_snapshot: Option<Vec<ColumnInput>>,
    /// Arrow names that are allowed to be used as group-by keys. None = all allowed.
    pub allowable_group_by: Option<Vec<String>>,
    pub mandatory_group_by: Vec<String>,
    pub available_agg_functions: Option<AvailableAggFunctions>,
    /// Restricts which date truncation levels appear in the column ⋮ menu.
    pub available_date_truncations: Option<AvailableDateTruncations>,
    /// Cached inferred date truncation options per arrow_name.
    pub date_truncation_cache: std::collections::HashMap<String, DateTruncationOptions>,
    pub last_menu_col: Option<usize>,
    /// (level_idx, first_leaf, last_leaf) for the last clicked parent span ⋮ button.
    pub last_span_menu: Option<(usize, usize, usize)>,
    /// Per-column format overrides keyed by arrow_name. None = clear format.
    pub format_overrides: std::collections::HashMap<String, Option<NumberFormat>>,
    /// Cached column min/max for gradient conditional formats. Keyed by arrow_name.
    pub column_stats: std::collections::HashMap<String, (f64, f64)>,
    /// Per-column conditional format overrides. These take priority over data_style.conditional_formats.
    pub conditional_format_overrides: std::collections::HashMap<String, Vec<crate::types::ConditionalRule>>,

    // ── Expand / collapse state ──────────────────────────────────────────
    /// Virtual row list: maps each rendered row index to actual data.
    /// Only populated in grouped mode.
    pub virtual_rows: Vec<VirtualRowRef>,
    /// Cached sub-level query results, keyed by the filter combination.
    pub expand_caches: std::collections::HashMap<ExpandCacheKey, ArrowDataSource>,
    /// Which group rows are currently expanded (identified by their cache key).
    pub expanded_keys: std::collections::HashSet<ExpandCacheKey>,
    /// Display indices of group-key columns in order (one per group key).
    /// Icon for depth-d row appears on group_key_display_cols[d].
    pub group_key_display_cols: Vec<usize>,
    /// Pre-computed mapping from display col index → raw-row rendering info.
    pub raw_col_mappings: Vec<RawColMapping>,
    /// Row index of the most recently hit expand toggle (set by on_mouse_down).
    pub pending_expand_row: Option<usize>,
    /// Combined group_by(all_keys) result cached for lazy in-memory partitioning.
    /// Populated during expand-all; cleared when group keys change or collapse-all.
    pub lazy_combined_data: Option<ArrowDataSource>,
}

impl GridState {
    pub fn new() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            columns: Vec::new(),
            mapped_columns: Vec::new(),
            rows: 0,
            cell_x_offset: 0,
            cell_y_offset: 0,
            translate_x: 0.0,
            translate_y: 0.0,
            header_height: DEFAULT_HEADER_HEIGHT,
            group_header_height: 0.0,
            freeze_columns: 0,
            freeze_trailing_rows: 0,
            row_height: DEFAULT_ROW_HEIGHT,
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
            col_slide_anim: None,
            swap_animation_duration_ms: DEFAULT_SWAP_ANIMATION_DURATION_MS,
            col_layout: ColumnLayout { entries: Vec::new(), span_menus: Vec::new(), leaf_y: 0.0, leaf_h: 0.0, total_header_height: 0.0 },
            group_by_state: GroupByState::default(),
            grouped_data: None,
            original_column_input_snapshot: None,
            allowable_group_by: None,
            mandatory_group_by: Vec::new(),
            available_agg_functions: None,
            available_date_truncations: None,
            date_truncation_cache: std::collections::HashMap::new(),
            original_column_overrides_snapshot: None,
            last_menu_col: None,
            last_span_menu: None,
            format_overrides: std::collections::HashMap::new(),
            column_stats: std::collections::HashMap::new(),
            conditional_format_overrides: std::collections::HashMap::new(),
            virtual_rows: Vec::new(),
            expand_caches: std::collections::HashMap::new(),
            expanded_keys: std::collections::HashSet::new(),
            group_key_display_cols: Vec::new(),
            raw_col_mappings: Vec::new(),
            pending_expand_row: None,
            lazy_combined_data: None,
        }
    }

    /// Compute and cache min/max stats for a numeric column by arrow_name.
    pub fn compute_column_stats(&mut self, arrow_name: &str) -> Option<(f64, f64)> {
        if let Some(&cached) = self.column_stats.get(arrow_name) {
            return Some(cached);
        }
        let data = self.active_data()?;
        let col_idx = data.schema().fields().iter().position(|f| f.name() == arrow_name)?;
        let (min, max) = data.column_min_max(col_idx)?;
        self.column_stats.insert(arrow_name.to_string(), (min, max));
        Some((min, max))
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

        // Pre-populate date truncation cache for all Utf8/date columns so that
        // get_format_options() can detect date-like string columns immediately.
        let date_col_names: Vec<String> = if let Some(ref data) = self.arrow_data {
            use arrow_schema::DataType;
            (0..data.num_columns())
                .filter(|&i| matches!(data.schema().field(i).data_type(),
                    DataType::Utf8 | DataType::LargeUtf8 |
                    DataType::Date32 | DataType::Date64 |
                    DataType::Timestamp(_, _)))
                .map(|i| data.column_name(i).to_string())
                .collect()
        } else {
            Vec::new()
        };
        for name in date_col_names {
            self.ensure_date_truncation_options(&name);
        }

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

        // Rebuild header_levels from parent_titles to merge adjacent same-title spans.
        // collect_leaves creates one span per ColumnInput node; recompute_header_levels
        // merges adjacent leaves with the same parent_titles[level] into one span.
        let mut resolved = resolved;
        resolved.recompute_header_levels();

        for (arrow_name, fmt) in &self.format_overrides {
            for leaf in &mut resolved.leaves {
                if leaf.arrow_name == *arrow_name {
                    match fmt {
                        Some(f) => {
                            leaf.data_style.get_or_insert_with(Default::default).number_format = Some(f.clone());
                        }
                        None => {
                            if let Some(ref mut ds) = leaf.data_style {
                                ds.number_format = None;
                            }
                        }
                    }
                }
            }
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
        let mut scrolled_source: Vec<MappedColumn> = Vec::new();

        for c in &self.mapped_columns {
            if c.sticky {
                sticky.push(c.clone());
            } else if c.source_index >= self.cell_x_offset {
                scrolled_source.push(c.clone());
            }
        }

        let sticky_width: f64 = sticky.iter().map(|c| c.width).sum();

        // Include non-sticky columns whose right edge is visible (draw_x + width > 0)
        // OR which lie between the left edge and the right canvas boundary.
        // We ALWAYS include columns that are partially behind the sticky area (draw_x < 0)
        // so that walk_columns can accumulate correct x positions — the diff/clip mechanism
        // in the renderers will mask the hidden portion.
        // We BREAK once a column's left edge is past the right canvas edge; all subsequent
        // columns are further right and invisible.
        let mut draw_x = sticky_width + self.translate_x;
        let mut result_scrolled: Vec<MappedColumn> = Vec::new();

        for c in &scrolled_source {
            if draw_x >= self.width {
                // Left edge is past the right canvas edge — all subsequent are further right.
                break;
            }
            // Include this column regardless of whether it is partially off the left edge.
            // The rendering layer clips columns hidden behind sticky columns via diff logic.
            result_scrolled.push(c.clone());
            draw_x += c.width;
        }

        sticky.extend(result_scrolled);
        sticky
    }

    pub fn get_cell_content(&self, col: i32, row: i32) -> GridCell {
        if col < 0 || row < 0 {
            return GridCell::loading();
        }

        let arrow_col = self.display_to_arrow_index(col as usize);

        if let Some(data) = self.active_data() {
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

    /// Apply a format override to a leaf column by arrow_name.
    /// Directly mutates the resolved leaf's data_style so the render path picks it up.
    pub fn apply_format_override(&mut self, arrow_name: &str, fmt: Option<NumberFormat>) {
        self.format_overrides.insert(arrow_name.to_string(), fmt.clone());
        if let Some(ref mut resolved) = self.resolved_columns {
            for leaf in &mut resolved.leaves {
                if leaf.arrow_name == arrow_name {
                    match &fmt {
                        Some(f) => {
                            leaf.data_style.get_or_insert_with(Default::default).number_format = Some(f.clone());
                        }
                        None => {
                            if let Some(ref mut ds) = leaf.data_style {
                                ds.number_format = None;
                            }
                        }
                    }
                }
            }
        }
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

    pub fn active_data(&self) -> Option<&ArrowDataSource> {
        self.grouped_data.as_ref().or(self.arrow_data.as_ref())
    }

    /// Build the grouped column ColumnInput tree from the current group_by_state.
    /// The result schema's field names (e.g. "cost_sum") are used as `name` fields.
    pub fn build_grouped_column_input(
        &self,
        original_inputs: &[ColumnInput],
        grouped_schema_names: &[String],
    ) -> Vec<ColumnInput> {
        let mut result: Vec<ColumnInput> = Vec::new();

        fn find_leaf_input<'a>(inputs: &'a [ColumnInput], arrow_name: &str) -> Option<&'a ColumnInput> {
            for input in inputs {
                if let Some(ref children) = input.children {
                    if let Some(found) = find_leaf_input(children, arrow_name) {
                        return Some(found);
                    }
                } else if input.name.as_deref() == Some(arrow_name) {
                    return Some(input);
                }
            }
            None
        }

        fn find_parent_chain(inputs: &[ColumnInput], arrow_name: &str, chain: &mut Vec<String>) -> bool {
            for input in inputs {
                if let Some(ref children) = input.children {
                    chain.push(input.display.clone().or(input.name.clone()).unwrap_or_default());
                    if find_parent_chain(children, arrow_name, chain) {
                        return true;
                    }
                    chain.pop();
                } else if input.name.as_deref() == Some(arrow_name) {
                    return true;
                }
            }
            false
        }

        // Return leaf arrow names in display (DFS) order from a ColumnInput tree.
        fn leaf_names_in_order(inputs: &[ColumnInput]) -> Vec<String> {
            let mut out = Vec::new();
            for input in inputs {
                match &input.children {
                    Some(children) => out.extend(leaf_names_in_order(children)),
                    None => if let Some(n) = &input.name { out.push(n.clone()); }
                }
            }
            out
        }

        // Wrap `inner` in parent nodes with the given display titles (outermost first).
        // Uses blank titles ("") for group-key parents so those header cells stay empty.
        fn wrap_in_parents(parent_titles: Vec<String>, inner: ColumnInput) -> ColumnInput {
            let mut current = inner;
            for title in parent_titles.into_iter().rev() {
                current = ColumnInput {
                    name: None,
                    display: Some(title),
                    init_width: None,
                    is_resizable: false,
                    header_style: None,
                    data_style: None,
                    children: Some(vec![current]),
                    agg_func: None,
                    group_by: None,
                    group_by_truncation: None,
                };
            }
            current
        }

        // Determine the max parent-chain depth across all original columns.
        let max_depth = leaf_names_in_order(original_inputs)
            .iter()
            .map(|name| {
                let mut chain = Vec::new();
                find_parent_chain(original_inputs, name, &mut chain);
                chain.len()
            })
            .max()
            .unwrap_or(0);

        // Group-key columns always use a fixed 3-level structure:
        //   Group → Source Column Name → Precision (or source name if no truncation)
        // Value columns use: original_parents → Column Name → Agg Function  (max_depth + 2 levels)
        // Both must reach the same total depth, so we pad with leading blanks as needed.
        //   target_depth = max(3, max_depth + 2)
        //   group-key leading blanks  = target_depth - 3
        //   value col leading blanks  = target_depth - (max_depth + 2)  [= 0 when max_depth >= 1]
        let target_depth = std::cmp::max(3, max_depth + 2);
        let gk_leading_blanks = target_depth.saturating_sub(3);
        let val_leading_blanks = target_depth.saturating_sub(max_depth + 2);

        // ── Group-by columns ──────────────────────────────────────────────────
        // Header structure:
        //   [""×gk_leading_blanks, "Group", source_display_name] → leaf: precision or source_name
        //
        // Example (max_depth=1, target_depth=3, gk_leading_blanks=0):
        //   parents = ["Group", "Created At"],  leaf.display = "Month"
        //   → Row 0: "Group"  Row 1: "Created At"  Row 2: "Month"
        let mut group_key_leaves: Vec<ColumnInput> = Vec::new();
        for key in &self.group_by_state.group_keys {
            let source = &key.arrow_name;
            if let Some(original) = find_leaf_input(original_inputs, source) {
                let source_display = original.display.as_deref().unwrap_or(source);
                let leaf_display = match key.truncation {
                    Some(t) => t.display_name().to_string(),
                    None    => source_display.to_string(),
                };
                let leaf = ColumnInput {
                    name: Some(key.result_name()),
                    display: Some(leaf_display),
                    init_width: original.init_width,
                    is_resizable: original.is_resizable,
                    header_style: original.header_style.clone(),
                    data_style: original.data_style.clone(),
                    children: None,
                    agg_func: None,
                    group_by: None,
                    group_by_truncation: None,
                };
                let mut parents: Vec<String> = vec!["".to_string(); gk_leading_blanks];
                parents.push("Group".to_string());
                parents.push(source_display.to_string());
                group_key_leaves.push(wrap_in_parents(parents, leaf));
            }
        }
        result.extend(group_key_leaves);

        // ── Value columns (aggregated), iterated in original display order ────
        let display_ordered_names = leaf_names_in_order(original_inputs);

        for col_name in &display_ordered_names {
            if self.group_by_state.has_exact_key(col_name, None) {
                continue;
            }
            let agg_fns = match self.group_by_state.agg_fns_for(col_name) {
                Some(fns) if !fns.is_empty() => fns,
                _ => continue,
            };

            let original = find_leaf_input(original_inputs, col_name);
            let display_name = original
                .and_then(|o| o.display.as_deref())
                .unwrap_or(col_name.as_str());
            let base_width = original.and_then(|o| o.init_width).unwrap_or(DEFAULT_COLUMN_WIDTH);
            let child_width = (base_width / agg_fns.len() as f64).max(MIN_AGG_CHILD_COLUMN_WIDTH);

            let agg_leaves: Vec<ColumnInput> = agg_fns.iter().filter_map(|agg| {
                let agg_col_name = agg.alias(col_name);
                if !grouped_schema_names.iter().any(|s| s == &agg_col_name) {
                    return None;
                }
                let data_style = if matches!(agg, AggregateFunction::Count) {
                    Some(crate::types::DataStyle {
                        number_format: Some(crate::types::NumberFormat::Integer),
                        align: Some(crate::types::ContentAlign::Right),
                        ..Default::default()
                    })
                } else {
                    original.and_then(|o| o.data_style.clone())
                };
                Some(ColumnInput {
                    name: Some(agg_col_name),
                    display: Some(agg.display_name().to_string()),
                    init_width: Some(child_width),
                    is_resizable: true,
                    header_style: None,
                    data_style,
                    children: None,
                    agg_func: None,
                    group_by: None,
                    group_by_truncation: None,
                })
            }).collect();

            if agg_leaves.is_empty() {
                continue;
            }

            let col_node = ColumnInput {
                name: None,
                display: Some(display_name.to_string()),
                init_width: None,
                is_resizable: false,
                header_style: original.and_then(|o| o.header_style.clone()),
                data_style: None,
                children: Some(agg_leaves),
                agg_func: None,
                group_by: None,
                group_by_truncation: None,
            };

            // Wrap in original parent hierarchy then prepend leading blanks to match target_depth
            let mut parent_chain: Vec<String> = Vec::new();
            find_parent_chain(original_inputs, col_name, &mut parent_chain);
            let mut padded_chain: Vec<String> = vec!["".to_string(); val_leading_blanks];
            padded_chain.extend(parent_chain);
            result.push(wrap_in_parents(padded_chain, col_node));
        }

        result
    }

    /// Enter grouped mode: snapshot columns, set aggregations for all non-key columns,
    /// run the query, and rebuild the column tree.
    pub async fn enter_group_by(&mut self, col_width: f64) -> Result<(), String> {
        let data = self.arrow_data.as_ref().ok_or("No data loaded")?;

        // Snapshot current column config for restore
        if self.original_column_input_snapshot.is_none() {
            self.original_column_input_snapshot = self.column_input.clone();
            self.original_column_overrides_snapshot = self.column_overrides.clone();
        }

        // Initialize aggregations for all non-key columns with Count.
        // Use original display order so value columns appear in the same order
        // as the pre-grouping grid rather than alphabetical/schema order.
        fn leaf_names_in_order_gb(inputs: &[crate::types::ColumnInput]) -> Vec<String> {
            let mut out = Vec::new();
            for input in inputs {
                match &input.children {
                    Some(children) => out.extend(leaf_names_in_order_gb(children)),
                    None => if let Some(n) = &input.name { out.push(n.clone()); }
                }
            }
            out
        }

        // Get leaf names in display order from the original column config if available,
        // otherwise fall back to schema order.
        let display_ordered: Vec<String> = if let Some(ref snap) = self.original_column_input_snapshot {
            leaf_names_in_order_gb(snap)
        } else if let Some(ref ovr) = self.original_column_overrides_snapshot {
            leaf_names_in_order_gb(ovr)
        } else {
            data.schema().fields().iter().map(|f| f.name().clone()).collect()
        };

        // Helper: get the first allowed agg function for a column, given the
        // availableAggregateFunctions restriction and the column's Arrow data type.
        fn first_allowed_agg(
            arrow_name: &str,
            available: &Option<AvailableAggFunctions>,
            data: &ArrowDataSource,
        ) -> AggregateFunction {
            let dtype = data.schema().fields().iter()
                .find(|f| f.name() == arrow_name)
                .map(|f| f.data_type().clone())
                .unwrap_or(arrow_schema::DataType::Utf8);
            let type_compat = AggregateFunction::compatible_with(&dtype);

            let allowed: Vec<AggregateFunction> = match available {
                None => type_compat,
                Some(AvailableAggFunctions::Global(list)) =>
                    type_compat.into_iter().filter(|f| list.contains(f)).collect(),
                Some(AvailableAggFunctions::PerColumn(map)) => {
                    if let Some(col_list) = map.get(arrow_name) {
                        type_compat.into_iter().filter(|f| col_list.contains(f)).collect()
                    } else {
                        type_compat
                    }
                }
            };
            // First in the allowed list, or Count as final fallback
            allowed.into_iter().next().unwrap_or(AggregateFunction::Count)
        }

        for name in &display_ordered {
            // Only skip if the raw column (no truncation) is a group key.
            // Truncated date group keys still leave the source column as a value column.
            if !self.group_by_state.has_exact_key(name, None)
                && self.group_by_state.agg_fns_for(name).is_none()
            {
                // Determine default agg from ColumnInput.agg_func, filtered through
                // availableAggregateFunctions. Fall back to first allowed otherwise.
                let init_fns: Vec<AggregateFunction> = {
                    // Find agg_func from original column config
                    fn find_agg_func<'a>(inputs: &'a [ColumnInput], arrow_name: &str) -> Option<&'a [AggregateFunction]> {
                        for input in inputs {
                            if let Some(ref children) = input.children {
                                if let Some(found) = find_agg_func(children, arrow_name) {
                                    return Some(found);
                                }
                            } else if input.name.as_deref() == Some(arrow_name) {
                                return input.agg_func.as_deref();
                            }
                        }
                        None
                    }
                    let original_inputs_ref = self.original_column_input_snapshot.as_deref()
                        .or(self.original_column_overrides_snapshot.as_deref());

                    let candidate_fns: Vec<AggregateFunction> = if let Some(inputs) = original_inputs_ref {
                        find_agg_func(inputs, name).map(|f| f.to_vec()).unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    if !candidate_fns.is_empty() {
                        // Filter through allowed list
                        let dtype = data.schema().fields().iter()
                            .find(|f| f.name() == name.as_str())
                            .map(|f| f.data_type().clone())
                            .unwrap_or(arrow_schema::DataType::Utf8);
                        let type_compat = AggregateFunction::compatible_with(&dtype);
                        let filtered: Vec<AggregateFunction> = candidate_fns.into_iter()
                            .filter(|f| {
                                type_compat.contains(f) && match &self.available_agg_functions {
                                    None => true,
                                    Some(AvailableAggFunctions::Global(list)) => list.contains(f),
                                    Some(AvailableAggFunctions::PerColumn(map)) =>
                                        map.get(name.as_str()).map_or(true, |l| l.contains(f)),
                                }
                            })
                            .collect();
                        if !filtered.is_empty() {
                            filtered
                        } else {
                            vec![first_allowed_agg(name, &self.available_agg_functions, data)]
                        }
                    } else {
                        vec![first_allowed_agg(name, &self.available_agg_functions, data)]
                    }
                };
                self.group_by_state.set_agg_fns(name, init_fns);
            }
        }

        self.run_group_by_query(col_width).await
    }

     pub async fn run_group_by_query_preserve_expand(&mut self, col_width: f64) -> Result<(), String> {
        self.run_group_by_query_inner(col_width, true).await
    }

     pub async fn run_group_by_query(&mut self, col_width: f64) -> Result<(), String> {
        self.run_group_by_query_inner(col_width, false).await
    }

     async fn run_group_by_query_inner(&mut self, col_width: f64, preserve_expand: bool) -> Result<(), String> {
        let data = self.arrow_data.as_ref().ok_or("No data loaded")?;
        // Group by only the first key at the top level. Deeper levels are fetched
        // on demand via filter_raw → group_by(remaining_keys) when the user expands.
        let top_level_keys = &self.group_by_state.group_keys[..1.min(self.group_by_state.group_keys.len())];
        let result = data.group_by(
            top_level_keys,
            &self.group_by_state.aggregations,
        ).await?;

        // Build the full display schema: ALL group key result names + aggregation result names.
        // The DataFusion result only has the first key, but the display tree needs all of them
        // so that sub-key columns appear in the header and can be rendered as expand targets.
        let all_group_key_names: Vec<String> = self.group_by_state.group_keys.iter()
            .map(|k| k.result_name())
            .collect();
        let agg_result_names: Vec<String> = result.schema().fields().iter()
            .filter(|f| !all_group_key_names.iter().any(|k| k == f.name()))
            .map(|f| f.name().clone())
            .collect();
        let mut grouped_schema_names: Vec<String> = all_group_key_names.clone();
        grouped_schema_names.extend(agg_result_names);

        self.rows = result.num_rows();
        self.grouped_data = Some(result);
        self.sort_state = SortState::default();

        // Get original inputs for building the grouped tree
        let original_inputs = self.original_column_input_snapshot
            .clone()
            .or_else(|| {
                let schema = data.schema().clone();
                Some(schema.fields().iter().map(|f| ColumnInput {
                    name: Some(f.name().clone()),
                    display: Some(f.name().clone()),
                    init_width: Some(col_width),
                    is_resizable: true,
                    header_style: None,
                    data_style: None,
                    children: None,
                    agg_func: None,
                    group_by: None,
                    group_by_truncation: None,
                }).collect())
            })
            .unwrap_or_default();

        let grouped_input = self.build_grouped_column_input(&original_inputs, &grouped_schema_names);
        self.column_input = Some(grouped_input);
        self.column_overrides = None;
        self.configure_columns(&grouped_schema_names, col_width)?;

        if !preserve_expand {
            self.expand_caches.clear();
            self.expanded_keys.clear();
            self.lazy_combined_data = None;
        }
        self.pending_expand_row = None;
        self.rebuild_expand_metadata();
        self.rebuild_virtual_rows();
        Ok(())
    }

    pub fn exit_group_by(&mut self) {
        self.group_by_state = GroupByState::default();
        self.grouped_data = None;
        self.column_input = self.original_column_input_snapshot.take();
        self.column_overrides = self.original_column_overrides_snapshot.take();

        self.virtual_rows.clear();
        self.expand_caches.clear();
        self.expanded_keys.clear();
        self.lazy_combined_data = None;
        self.group_key_display_cols.clear();
        self.raw_col_mappings.clear();

        if let Some(ref data) = self.arrow_data {
            self.rows = data.num_rows();
            let names: Vec<String> = data.schema().fields().iter()
                .map(|f| f.name().clone())
                .collect();
            let _ = self.configure_columns(&names, DEFAULT_COLUMN_WIDTH);
        }
    }

    /// Compute the expand metadata from the current resolved_columns:
    /// - which display column is the last group-key column
    /// - how many group-key columns there are
    /// - the raw_col_mappings table
    pub fn rebuild_expand_metadata(&mut self) {
        let resolved = match &self.resolved_columns { Some(r) => r, None => return };
        let group_keys = &self.group_by_state.group_keys;
        let arrow_data = match &self.arrow_data { Some(d) => d, None => return };

        let mut group_key_display_cols: Vec<usize> = Vec::new();
        let mut mappings: Vec<RawColMapping> = Vec::new();

        // Track parent groups (same source column, multiple agg fns)
        // by looking at consecutive leaves with the same parent display title.
        let leaves = &resolved.leaves;
        let n = leaves.len();

        // Build a map: source col name -> ordered agg sibling display indices.
        // Use the exact alias match (same logic as source_name lookup) so we never
        // accidentally include group-key result columns (e.g. "created_at_month"
        // would wrongly match a starts_with("created_at") check).
        let mut agg_siblings: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        for leaf in leaves {
            let name = &leaf.arrow_name;
            for (col_name, fns) in &self.group_by_state.aggregations {
                if fns.iter().any(|f| f.alias(col_name) == *name) {
                    agg_siblings.entry(col_name.clone())
                        .or_default()
                        .push(leaf.display_index);
                    break;
                }
            }
        }

        for i in 0..n {
            let leaf = &leaves[i];
            let arrow_name = &leaf.arrow_name;

            // Is this a group-key column?
            let is_gk = group_keys.iter().any(|k| k.result_name() == *arrow_name || (k.truncation.is_none() && k.arrow_name == *arrow_name));

            if is_gk {
                group_key_display_cols.push(leaf.display_index);
                mappings.push(RawColMapping {
                    display_idx: leaf.display_index,
                    is_group_key: true,
                    source_arrow_idx: None,
                    merge_span: 0,
                    is_merge_skip: false,
                });
                continue;
            }

            // Find source column name (strip agg suffix)
            let source_name = self.group_by_state.aggregations.iter()
                .find_map(|(col_name, fns)| {
                    if fns.iter().any(|f| f.alias(col_name) == *arrow_name) {
                        Some(col_name.clone())
                    } else {
                        None
                    }
                });

            let source_arrow_idx = source_name.as_deref().and_then(|sn| {
                arrow_data.schema().fields().iter().position(|f| f.name() == sn)
            });

            // Determine merge role
            let (merge_span, is_merge_skip) = if let Some(sn) = &source_name {
                if let Some(siblings) = agg_siblings.get(sn) {
                    let pos = siblings.iter().position(|&d| d == leaf.display_index);
                    match pos {
                        Some(0) => (siblings.len() - 1, false), // first: spans the rest
                        Some(_) => (0, true),                    // non-first: skip
                        None => (0, false),
                    }
                } else {
                    (0, false)
                }
            } else {
                (0, false)
            };

            mappings.push(RawColMapping {
                display_idx: leaf.display_index,
                is_group_key: false,
                source_arrow_idx,
                merge_span,
                is_merge_skip,
            });
        }

        self.group_key_display_cols = group_key_display_cols;
        self.raw_col_mappings = mappings;
    }

    /// Rebuild the flat virtual_rows list from grouped_data + expanded_keys + expand_caches.
    /// Uses recursive depth-first traversal supporting N group keys.
    pub fn rebuild_virtual_rows(&mut self) {
        let n_top = self.grouped_data.as_ref().map(|d| d.num_rows()).unwrap_or(0);
        let n_keys = self.group_by_state.group_keys.len();
        let mut rows: Vec<VirtualRowRef> = Vec::new();

        for top_idx in 0..n_top {
            let top_key: ExpandCacheKey = Vec::new(); // depth-0 rows have empty parent key
            let row_cache_key = self.cache_key_for_aggregate(&top_key, top_idx, 0);
            rows.push(VirtualRowRef::Aggregate { depth: 0, row_idx: top_idx, cache_key: top_key });
            self.append_children(&mut rows, &row_cache_key, 0, n_keys);
        }

        self.rows = rows.len();
        self.virtual_rows = rows;
    }

    /// Append expanded children for an aggregate row at `depth` with `row_cache_key`.
    fn append_children(&self, rows: &mut Vec<VirtualRowRef>, row_cache_key: &ExpandCacheKey, depth: usize, n_keys: usize) {
        if !self.expanded_keys.contains(row_cache_key) { return; }

        let next_depth = depth + 1;
        let is_leaf = next_depth >= n_keys;

        if let Some(child_data) = self.expand_caches.get(row_cache_key) {
            let n_children = child_data.num_rows();
            for child_idx in 0..n_children {
                let child_key = self.cache_key_for_aggregate(row_cache_key, child_idx, next_depth);
                if is_leaf {
                    rows.push(VirtualRowRef::Raw { source_row: child_idx, parent_key: row_cache_key.clone() });
                } else {
                    rows.push(VirtualRowRef::Aggregate { depth: next_depth, row_idx: child_idx, cache_key: row_cache_key.clone() });
                    self.append_children(rows, &child_key, next_depth, n_keys);
                }
            }
        } else {
            // Cache not populated yet — insert a single Pending placeholder.
            rows.push(VirtualRowRef::Pending { cache_key: row_cache_key.clone(), depth: next_depth });
        }
    }

    /// Build the cache key that identifies an aggregate row, by appending this row's
    /// group-key value to the parent key. Used both in rebuild_virtual_rows and toggle_expand.
    fn cache_key_for_aggregate(&self, parent_key: &ExpandCacheKey, row_idx: usize, depth: usize) -> ExpandCacheKey {
        let group_keys = &self.group_by_state.group_keys;
        let gk = match group_keys.get(depth) { Some(k) => k, None => return parent_key.clone() };
        let result_name = gk.result_name();

        // depth=0 → read from grouped_data; depth>0 → read from expand_caches[parent_key]
        let data: Option<&ArrowDataSource> = if depth == 0 {
            self.grouped_data.as_ref()
        } else {
            self.expand_caches.get(parent_key)
        };

        let display = data
            .and_then(|d| d.schema().fields().iter().position(|f| f.name().as_str() == result_name).map(|ci| d.get_cell_raw_text(ci, row_idx)))
            .unwrap_or_default();

        let mut key = parent_key.clone();
        key.push((result_name, display));
        key
    }

    /// Toggle expand/collapse for a virtual row at the given depth.
    /// `depth_to_expand` is the depth at which the user clicked (which group-key column).
    pub async fn toggle_expand(&mut self, virtual_row: usize, depth_to_expand: usize) -> Result<(), String> {
        let vrow = self.virtual_rows.get(virtual_row).cloned()
            .ok_or_else(|| format!("Invalid virtual row {}", virtual_row))?;

        let group_keys = self.group_by_state.group_keys.clone();
        let aggregations = self.group_by_state.aggregations.clone();
        let n_keys = group_keys.len();

        let (row_depth, row_idx, parent_cache_key) = match &vrow {
            VirtualRowRef::Aggregate { depth, row_idx, cache_key } => (*depth, *row_idx, cache_key.clone()),
            VirtualRowRef::Raw { .. } | VirtualRowRef::Pending { .. } => return Ok(()),
        };

        // Only allow expanding at the row's own depth level
        if depth_to_expand != row_depth { return Ok(()); }

        // The cache key that identifies this row (depth-aware)
        let row_key = self.cache_key_for_aggregate(&parent_cache_key, row_idx, row_depth);

        if self.expanded_keys.contains(&row_key) {
            // Collapse: remove this key and all descendants
            let to_remove: Vec<_> = self.expanded_keys.iter()
                .filter(|k| k.starts_with(row_key.as_slice()))
                .cloned()
                .collect();
            for k in to_remove { self.expanded_keys.remove(&k); }
        } else {
            // Expand: fetch child data if not cached
            if !self.expand_caches.contains_key(&row_key) {
                let arrow_data = self.arrow_data.as_ref().ok_or("No source data")?;

                // Build filter predicates from row_key entries
                let filters: Vec<(crate::types::DateGroupKey, String)> = row_key.iter()
                    .filter_map(|(col_name, val)| {
                        group_keys.iter().find(|k| k.result_name() == *col_name)
                            .map(|k| (k.clone(), val.clone()))
                    })
                    .collect();

                let next_depth = row_depth + 1;
                let is_leaf = next_depth >= n_keys;

                let filtered = arrow_data.filter_raw(&filters).await?;
                let result = if is_leaf {
                    filtered
                } else {
                    filtered.group_by(&group_keys[next_depth..next_depth + 1], &aggregations).await?
                };

                self.expand_caches.insert(row_key.clone(), result);
            }
            self.expanded_keys.insert(row_key);
        }

        self.rebuild_virtual_rows();
        Ok(())
    }

    /// Compute and cache date truncation options for a column.
    /// If already cached with is_estimate=false, this is a no-op.
    pub fn ensure_date_truncation_options(&mut self, arrow_name: &str) {
        use crate::types::DateTruncation;

        if let Some(cached) = self.date_truncation_cache.get(arrow_name) {
            if !cached.is_estimate { return; }
        }
        let data = match self.active_data() { Some(d) => d, None => return };
        let col_idx = match data.schema().fields().iter().position(|f| f.name() == arrow_name) {
            Some(i) => i, None => return,
        };
        let (available, is_estimate) = data.infer_date_truncations(col_idx);

        // Apply availableDateTruncations restriction
        let filtered: Vec<DateTruncation> = match &self.available_date_truncations {
            None => available,
            Some(crate::types::AvailableDateTruncations::Global(allowed)) =>
                available.into_iter().filter(|t| allowed.contains(t)).collect(),
            Some(crate::types::AvailableDateTruncations::PerColumn(map)) => {
                if let Some(allowed) = map.get(arrow_name) {
                    available.into_iter().filter(|t| allowed.contains(t)).collect()
                } else { available }
            }
        };
        self.date_truncation_cache.insert(arrow_name.to_string(),
            crate::types::DateTruncationOptions { available: filtered, is_estimate });
    }

    /// Recompute date truncation options for stale (estimated) caches.
    /// Called after a sort so we get accurate smallest-tick data.
    pub fn recompute_stale_date_truncations(&mut self) {
        let stale: Vec<String> = self.date_truncation_cache
            .iter()
            .filter_map(|(k, v)| if v.is_estimate { Some(k.clone()) } else { None })
            .collect();
        for name in stale {
            // Mark as stale by removing — will be recomputed lazily or eagerly below
            self.date_truncation_cache.remove(&name);
            self.ensure_date_truncation_options(&name);
        }
    }

    pub fn recompute_layout(&mut self) {
        
        let effective = self.effective_columns();

        // Build the list of value (aggregated) column arrow names for span menu filtering.
        // Only pass Some(...) when grouping is active; otherwise span menus are suppressed.
        let grouped_value_col_names: Option<Vec<String>> = if self.group_by_state.is_active() {
            Some(
                self.group_by_state.aggregations
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect(),
            )
        } else {
            None
        };

        self.col_layout = layout::compute_column_layout(
            &effective,
            self.translate_x,
            self.header_height,
            self.group_header_height,
            self.resolved_columns.as_ref(),
            grouped_value_col_names.as_deref(),
        );
    }

    pub fn render(&mut self, ctx: &mut crate::canvas::CanvasCtx) {
        self.recompute_layout();
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

        let freeze_trailing_rows = self.freeze_trailing_rows;
        let has_append_row = self.has_append_row;
        let width = self.width;
        let height = self.height;
        let resolved = self.resolved_columns.clone();
        let resize_state = self.resize_state.clone();
        let col_drag = self.col_drag.clone();
        let col_layout = self.col_layout.clone();
        let conditional_format_overrides = self.conditional_format_overrides.clone();
        let column_stats = self.column_stats.clone();

        let grouped_data_ref = self.grouped_data.as_ref();
        let arrow_data_ref = self.arrow_data.as_ref();
        let cb_ref = self.get_cell_content_cb.as_ref();
        let resolved_ref = &resolved;
        let virtual_rows_ref = &self.virtual_rows;
        let expand_caches_ref = &self.expand_caches;
        let raw_col_mappings_ref = &self.raw_col_mappings;
        let is_grouped = grouped_data_ref.is_some() && !virtual_rows_ref.is_empty();

        let get_cell_content = |col: i32, row: i32| -> GridCell {
            if col < 0 || row < 0 { return GridCell::loading(); }
            let display_col = col as usize;

            if is_grouped {
                let vrow = match virtual_rows_ref.get(row as usize) {
                    Some(v) => v,
                    None => return GridCell::loading(),
                };

                match vrow {
                    VirtualRowRef::Aggregate { depth, row_idx, cache_key } => {
                        let data: Option<&ArrowDataSource> = if *depth == 0 {
                            grouped_data_ref
                        } else {
                            expand_caches_ref.get(cache_key)
                        };
                        let data = match data { Some(d) => d, None => return GridCell::loading() };
                        let col_name = resolved_ref.as_ref()
                            .and_then(|r| r.leaves.get(display_col))
                            .map(|l| l.arrow_name.as_str())
                            .unwrap_or("");
                        let arrow_col = data.schema().fields().iter()
                            .position(|f| f.name() == col_name)
                            .unwrap_or(usize::MAX);
                        if arrow_col == usize::MAX {
                            GridCell::Text { data: String::new(), display_data: None, content_align: None }
                        } else {
                            data.get_cell(arrow_col, *row_idx)
                        }
                    }
                    VirtualRowRef::Pending { .. } => {
                        GridCell::Loading { skeleton_width: None }
                    }
                    VirtualRowRef::Raw { source_row, parent_key } => {
                        let mapping = raw_col_mappings_ref.get(display_col);
                        match mapping {
                            None => GridCell::loading(),
                            Some(m) if m.is_group_key => {
                                GridCell::Text { data: String::new(), display_data: None, content_align: None }
                            }
                            Some(m) if m.is_merge_skip => {
                                GridCell::Skip { span: 1 }
                            }
                            Some(m) => {
                                let raw_data = expand_caches_ref.get(parent_key).or(arrow_data_ref);
                                match (raw_data, m.source_arrow_idx) {
                                    (Some(d), Some(src_col)) => d.get_cell(src_col, *source_row),
                                    _ => GridCell::loading(),
                                }
                            }
                        }
                    }
                }
            } else {
                let arrow_col = if let Some(ref r) = resolved_ref {
                    r.leaves.get(display_col).map(|l| l.arrow_index).unwrap_or(display_col)
                } else {
                    display_col
                };
                let arrow_ref = grouped_data_ref.or(arrow_data_ref);
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
            }
        };

        let get_group_details = |_name: &str| GroupDetails::default();

        // Pre-compute which virtual row indices are expanded and aggregate row depths.
        let expanded_vrow_indices: std::collections::HashSet<usize> = self.virtual_rows.iter()
            .enumerate()
            .filter_map(|(vi, vrow)| {
                if let VirtualRowRef::Aggregate { depth, row_idx, cache_key } = vrow {
                    let row_key = self.cache_key_for_aggregate(cache_key, *row_idx, *depth);
                    if self.expanded_keys.contains(&row_key) { Some(vi) } else { None }
                } else { None }
            })
            .collect();
        let is_row_expanded = move |vrow_idx: usize| expanded_vrow_indices.contains(&vrow_idx);

        // For each virtual row, record which group-key display column should show the icon.
        let group_key_display_cols_snap = self.group_key_display_cols.clone();
        let vrow_icon_cols: Vec<Option<usize>> = self.virtual_rows.iter()
            .map(|vrow| match vrow {
                VirtualRowRef::Aggregate { depth, .. } => group_key_display_cols_snap.get(*depth).copied(),
                VirtualRowRef::Raw { .. } | VirtualRowRef::Pending { .. } => None,
            })
            .collect();
        let vrow_icon_cols_ref = vrow_icon_cols.clone();

        let virtual_rows_for_agg = self.virtual_rows.clone();
        let is_grouped_mode = !virtual_rows_for_agg.is_empty();
        let is_aggregate_row = move |vrow_idx: usize| -> bool {
            if !is_grouped_mode { return false; }
            virtual_rows_for_agg.get(vrow_idx).map(|v| v.is_aggregate()).unwrap_or(false)
        };
        // show_expand_icon(col, vrow_idx) — true only for the icon column at this row's depth
        let show_expand_icon_fn = move |display_col: usize, vrow_idx: usize| -> bool {
            vrow_icon_cols_ref.get(vrow_idx).and_then(|c| *c).map(|c| c == display_col).unwrap_or(false)
        };
        let group_key_display_cols_for_layout = self.group_key_display_cols.clone();

        // For the header expand icons:
        let gkdc_for_header = self.group_key_display_cols.clone();
        let expanded_keys_for_header = self.expanded_keys.clone();
        let grouped_data_rows = self.grouped_data.as_ref().map(|d| d.num_rows()).unwrap_or(0);
        let expand_caches_keys: Vec<ExpandCacheKey> = self.expand_caches.keys().cloned().collect();

        let is_group_key_col = move |display_col: usize| -> bool {
            gkdc_for_header.contains(&display_col)
        };
        // A depth is "all expanded" if every top-level row has its cache key in expanded_keys.
        // We approximate by checking if the number of expanded keys at depth `d` equals
        // the number of top-level rows (for depth 0) or sub-rows (for depth 1+).
        // Simpler: depth is all-expanded when expanded_keys is non-empty and covers all rows
        // at that depth. For now we check: for depth 0, all grouped_data rows are expanded.
        let is_depth_all_expanded = move |display_col: usize| -> bool {
            // Find which depth this display_col corresponds to
            // (it's the index in gkdc where gkdc[depth] == display_col, but gkdc was moved)
            // We encode depth differently: depth 0 = first group key col.
            // Count keys at depth 0 (single-entry cache keys).
            let depth0_expanded = expanded_keys_for_header.iter()
                .filter(|k| k.len() == 1)
                .count();
            grouped_data_rows > 0 && depth0_expanded >= grouped_data_rows
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
            cell_y_offset,
            translate_x,
            translate_y,
            freeze_trailing_rows,
            has_append_row,
            &selection,
            &sort_state,
            &theme,
            is_focused,
            true,
            &get_cell_content,
            &get_group_details,
            resolved.as_ref(),
            resize_state.as_ref(),
            col_drag.as_ref(),
            &col_layout,
            &conditional_format_overrides,
            &column_stats,
            &show_expand_icon_fn,
            &is_row_expanded,
            &is_aggregate_row,
            &is_group_key_col,
            &is_depth_all_expanded,
        );



        // Paint slide animation on top of the rendered grid
        let anim_done = if let Some(ref anim) = self.col_slide_anim {
            let now = js_sys::Date::now();
            if anim.is_done(now) {
                true
            } else {
                let p = anim.progress_at(now);
                let ax = anim.a_start_x * (1.0 - p) + anim.a_end_x * p;
                let bx = anim.b_start_x * (1.0 - p) + anim.b_end_x * p;
                ctx.draw_canvas_at(&anim.canvas_a, ax, anim.y);
                ctx.draw_canvas_at(&anim.canvas_b, bx, anim.y);
                false
            }
        } else {
            false
        };
        if anim_done {
            self.col_slide_anim = None;
        }
    }

    pub fn clean_finished_animation(&mut self) {
        if let Some(ref anim) = self.col_slide_anim {
            if anim.is_done(js_sys::Date::now()) {
                self.col_slide_anim = None;
            }
        }
    }
}
