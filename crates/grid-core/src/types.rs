use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn intersects(&self, other: &Rectangle) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    pub col: i32,
    pub row: i32,
}

impl Item {
    pub fn new(col: i32, row: i32) -> Self {
        Self { col, row }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum GridCell {
    #[serde(rename = "text")]
    Text {
        data: String,
        #[serde(rename = "displayData")]
        display_data: Option<String>,
        #[serde(rename = "contentAlign")]
        content_align: Option<ContentAlign>,
    },
    #[serde(rename = "number")]
    Number {
        data: Option<f64>,
        #[serde(rename = "displayData")]
        display_data: Option<String>,
        #[serde(rename = "contentAlign")]
        content_align: Option<ContentAlign>,
    },
    #[serde(rename = "loading")]
    Loading {
        #[serde(rename = "skeletonWidth")]
        skeleton_width: Option<f64>,
    },
    /// Sentinel: this cell is merged into the cell `span` columns to its left.
    /// draw_cells skips rendering it and extends the left neighbour's draw rect.
    #[serde(rename = "skip")]
    Skip { span: usize },
}

/// Identifies a specific group combination, used as a cache key and filter spec.
/// Each entry is (result_col_name, display_string_value), e.g. ("created_at_month", "2022-01-01 00:00:00").
pub type ExpandCacheKey = Vec<(String, String)>;

/// Reference to a single virtual row in the expanded grid view.
#[derive(Debug, Clone)]
pub enum VirtualRowRef {
    /// An aggregate row at a given depth level.
    /// depth=0 → rows from `grouped_data` (grouped by key[0] only).
    /// depth=1 → rows from `expand_caches[cache_key]` (grouped by key[0..2]), etc.
    /// `cache_key` is the filter combination used to produce this node's data source.
    /// For depth=0 rows, `cache_key` is empty.
    Aggregate {
        depth: usize,
        row_idx: usize,
        cache_key: ExpandCacheKey,
    },
    /// A raw source-data row (leaf level, no further expansion possible).
    Raw { source_row: usize, parent_key: ExpandCacheKey },
}

impl VirtualRowRef {
    pub fn is_aggregate(&self) -> bool {
        matches!(self, VirtualRowRef::Aggregate { .. })
    }
    pub fn depth(&self) -> usize {
        match self {
            VirtualRowRef::Aggregate { depth, .. } => *depth,
            VirtualRowRef::Raw { .. } => usize::MAX,
        }
    }
}

impl GridCell {
    pub fn loading() -> Self {
        GridCell::Loading { skeleton_width: None }
    }

    pub fn text(data: &str) -> Self {
        GridCell::Text {
            data: data.to_string(),
            display_data: None,
            content_align: None,
        }
    }

    pub fn number(data: f64) -> Self {
        GridCell::Number {
            data: Some(data),
            display_data: None,
            content_align: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridColumn {
    pub title: String,
    pub width: f64,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridSelection {
    pub current: Option<CurrentSelection>,
    pub columns: Vec<i32>,
    pub rows: Vec<i32>,
    /// Non-contiguous additional cells from Ctrl+Click.
    #[serde(default)]
    pub ctrl_cells: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentSelection {
    /// The anchor cell (where shift-extend and drag started from).
    pub cell: Item,
    /// The selected range. For a single cell: width=1, height=1.
    /// For a drag or shift+click: spans from anchor to the opposite corner.
    pub range: Rectangle,
}

impl GridSelection {
    pub fn single(col: i32, row: i32) -> Self {
        Self {
            current: Some(CurrentSelection {
                cell: Item::new(col, row),
                range: Rectangle::new(col as f64, row as f64, 1.0, 1.0),
            }),
            columns: Vec::new(),
            rows: Vec::new(),
            ctrl_cells: Vec::new(),
        }
    }

    pub fn is_multi(&self) -> bool {
        if let Some(ref cur) = self.current {
            if cur.range.width > 1.0 || cur.range.height > 1.0 {
                return true;
            }
        }
        !self.ctrl_cells.is_empty()
    }
}

impl Default for GridSelection {
    fn default() -> Self {
        Self {
            current: None,
            columns: Vec::new(),
            rows: Vec::new(),
            ctrl_cells: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NumberFormat {
    Accounting {
        #[serde(default = "default_two")]
        decimals: u32,
    },
    Currency {
        #[serde(default = "default_dollar")]
        symbol: String,
        #[serde(default = "default_two")]
        decimals: u32,
    },
    Percent {
        #[serde(default = "default_one")]
        decimals: u32,
    },
    Decimal {
        #[serde(default = "default_two")]
        decimals: u32,
    },
    Integer,
    Date {
        #[serde(default = "default_date_format")]
        format: String,
    },
    DateTime {
        #[serde(default = "default_datetime_format")]
        format: String,
    },
}

fn default_two() -> u32 { 2 }
fn default_one() -> u32 { 1 }
fn default_dollar() -> String { "$".to_string() }
fn default_date_format() -> String { "%Y-%m-%d".to_string() }
fn default_datetime_format() -> String { "%Y-%m-%d %H:%M:%S".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConditionalRule {
    GreaterThan { value: f64, style: CellStyleOverride },
    LessThan { value: f64, style: CellStyleOverride },
    Equal { value: f64, style: CellStyleOverride },
    Between { min: f64, max: f64, style: CellStyleOverride },
    Contains { value: String, style: CellStyleOverride },
    IsNull { style: CellStyleOverride },
    IsNotNull { style: CellStyleOverride },
    Percentile {
        #[serde(default = "default_low_pct")]
        low: f64,
        #[serde(default = "default_high_pct")]
        high: f64,
        low_style: CellStyleOverride,
        mid_style: Option<CellStyleOverride>,
        high_style: CellStyleOverride,
    },
    /// Interpolate bg_color between two colors.
    /// min_value/max_value can override auto-detected column stats.
    /// Values outside the range clamp to the low/high color.
    Gradient {
        #[serde(rename = "lowColor")]
        low_color: String,
        #[serde(rename = "highColor")]
        high_color: String,
        #[serde(rename = "minValue", default)]
        min_value: Option<f64>,
        #[serde(rename = "maxValue", default)]
        max_value: Option<f64>,
    },
    /// Map specific cell display values to background colors.
    ValueColor {
        rules: Vec<ValueColorRule>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueColorRule {
    pub value: String,
    pub bg_color: String,
}

fn default_low_pct() -> f64 { 0.25 }
fn default_high_pct() -> f64 { 0.75 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellStyleOverride {
    pub color: Option<String>,
    pub bg_color: Option<String>,
    pub font: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderStyle {
    pub font: Option<String>,
    pub color: Option<String>,
    pub bg_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataStyle {
    pub font: Option<String>,
    pub color: Option<String>,
    pub bg_color: Option<String>,
    pub align: Option<ContentAlign>,
    pub number_format: Option<NumberFormat>,
    pub conditional_formats: Option<Vec<ConditionalRule>>,
}

impl Default for DataStyle {
    fn default() -> Self {
        Self {
            font: None,
            color: None,
            bg_color: None,
            align: None,
            number_format: None,
            conditional_formats: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInput {
    pub name: Option<String>,
    pub display: Option<String>,
    pub init_width: Option<f64>,
    #[serde(default = "default_true")]
    pub is_resizable: bool,
    pub header_style: Option<HeaderStyle>,
    pub data_style: Option<DataStyle>,
    pub children: Option<Vec<ColumnInput>>,
    /// Initial aggregation function(s) for this column when grouped mode is entered.
    /// If absent, falls back to the first function in `availableAggregateFunctions`
    /// (or Count if no restriction is set).
    #[serde(default)]
    pub agg_func: Option<Vec<AggregateFunction>>,
    /// If set, this column is automatically a group-by key on load.
    /// The value is the relative ordering among group-by keys (0 = leftmost).
    #[serde(default)]
    pub group_by: Option<u32>,
    /// For date/datetime group-by columns: truncation level to apply.
    /// Only meaningful when group_by is also set.
    #[serde(default)]
    pub group_by_truncation: Option<DateTruncation>,
}

fn default_true() -> bool { true }

/// Restricts which aggregation functions appear in the column ⋮ menu.
/// `Global(fns)` applies the same list to all columns.
/// `PerColumn(map)` applies per column by `arrow_name`; columns not in the map
/// receive all type-compatible functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AvailableAggFunctions {
    Global(Vec<AggregateFunction>),
    PerColumn(std::collections::HashMap<String, Vec<AggregateFunction>>),
}

#[derive(Debug, Clone)]
pub struct ResizeState {
    pub column_display_index: usize,
    pub start_x: f64,
    pub start_width: f64,
    pub current_x: f64,
}

#[derive(Debug, Clone)]
pub struct ColDragState {
    pub col_display_index: usize,
    pub col_title: String,
    pub start_x: f64,
    pub start_y: f64,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub prev_mouse_x: f64,
    pub has_activated: bool,
}

pub struct ColSlideAnimation {
    pub canvas_a: web_sys::HtmlCanvasElement,
    pub canvas_b: web_sys::HtmlCanvasElement,
    pub a_start_x: f64,
    pub b_start_x: f64,
    pub a_end_x: f64,
    pub b_end_x: f64,
    pub y: f64,
    pub start_time_ms: f64,
    pub duration_ms: f64,
}

impl ColSlideAnimation {
    pub fn progress_at(&self, now_ms: f64) -> f64 {
        let t = ((now_ms - self.start_time_ms) / self.duration_ms).clamp(0.0, 1.0);
        if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
    }

    pub fn is_done(&self, now_ms: f64) -> bool {
        now_ms >= self.start_time_ms + self.duration_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AggregateFunction {
    Count,
    Sum,
    Min,
    Max,
    Mean,
}

impl AggregateFunction {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Count => "Count",
            Self::Sum => "Sum",
            Self::Min => "Min",
            Self::Max => "Max",
            Self::Mean => "Mean",
        }
    }

    pub fn alias(&self, col_name: &str) -> String {
        match self {
            Self::Count => format!("{}_count", col_name),
            Self::Sum => format!("{}_sum", col_name),
            Self::Min => format!("{}_min", col_name),
            Self::Max => format!("{}_max", col_name),
            Self::Mean => format!("{}_mean", col_name),
        }
    }

    pub fn compatible_with(dtype: &arrow_schema::DataType) -> Vec<AggregateFunction> {
        use arrow_schema::DataType::*;
        let mut fns = vec![AggregateFunction::Count];
        match dtype {
            Int8 | Int16 | Int32 | Int64
            | UInt8 | UInt16 | UInt32 | UInt64
            | Float16 | Float32 | Float64 => {
                fns.extend([AggregateFunction::Sum, AggregateFunction::Min, AggregateFunction::Max, AggregateFunction::Mean]);
            }
            Utf8 | LargeUtf8 | Boolean => {
                fns.extend([AggregateFunction::Min, AggregateFunction::Max]);
            }
            Date32 | Date64 | Timestamp(..) => {
                fns.extend([AggregateFunction::Min, AggregateFunction::Max]);
            }
            _ => {}
        }
        fns
    }
}

/// Truncation granularity for date/datetime group-by keys.
/// Mirrors DataFusion's date_trunc() precision strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateTruncation {
    Year, Quarter, Month, Week, Day,
    Hour, Minute, Second,
    Millisecond, Microsecond, Nanosecond,
}

impl DateTruncation {
    /// DataFusion date_trunc() precision string.
    pub fn precision(&self) -> &'static str {
        match self {
            Self::Year => "year", Self::Quarter => "quarter", Self::Month => "month",
            Self::Week => "week", Self::Day => "day", Self::Hour => "hour",
            Self::Minute => "minute", Self::Second => "second",
            Self::Millisecond => "millisecond", Self::Microsecond => "microsecond",
            Self::Nanosecond => "nanosecond",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Year => "Year", Self::Quarter => "Quarter", Self::Month => "Month",
            Self::Week => "Week", Self::Day => "Day", Self::Hour => "Hour",
            Self::Minute => "Minute", Self::Second => "Second",
            Self::Millisecond => "Millisecond", Self::Microsecond => "Microsecond",
            Self::Nanosecond => "Nanosecond",
        }
    }

    /// Short suffix used in derived column names, e.g. "created_at_month".
    pub fn alias_suffix(&self) -> &'static str {
        match self {
            Self::Year => "year", Self::Quarter => "quarter", Self::Month => "month",
            Self::Week => "week", Self::Day => "day", Self::Hour => "hour",
            Self::Minute => "minute", Self::Second => "second",
            Self::Millisecond => "ms", Self::Microsecond => "us", Self::Nanosecond => "ns",
        }
    }

    /// Approximate duration in microseconds (used for smallest-tick comparisons).
    pub fn duration_micros(&self) -> i64 {
        match self {
            Self::Nanosecond  => 1,
            Self::Microsecond => 1,
            Self::Millisecond => 1_000,
            Self::Second      => 1_000_000,
            Self::Minute      => 60_000_000,
            Self::Hour        => 3_600_000_000,
            Self::Day         => 86_400_000_000,
            Self::Week        => 604_800_000_000,
            Self::Month       => 2_592_000_000_000,
            Self::Quarter     => 7_776_000_000_000,
            Self::Year        => 31_536_000_000_000,
        }
    }

    /// All truncations from coarsest to finest (used for inference).
    pub fn all_coarsest_first() -> &'static [DateTruncation] {
        &[
            DateTruncation::Year, DateTruncation::Quarter, DateTruncation::Month,
            DateTruncation::Week, DateTruncation::Day, DateTruncation::Hour,
            DateTruncation::Minute, DateTruncation::Second,
            DateTruncation::Millisecond, DateTruncation::Microsecond, DateTruncation::Nanosecond,
        ]
    }
}

/// A single group-by key, optionally with a date truncation applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateGroupKey {
    /// Source Arrow column name (e.g. "created_at").
    pub arrow_name: String,
    /// Truncation for date/datetime columns. None = use raw value.
    pub truncation: Option<DateTruncation>,
}

impl DateGroupKey {
    pub fn raw(arrow_name: impl Into<String>) -> Self {
        Self { arrow_name: arrow_name.into(), truncation: None }
    }
    pub fn truncated(arrow_name: impl Into<String>, t: DateTruncation) -> Self {
        Self { arrow_name: arrow_name.into(), truncation: Some(t) }
    }
    /// The column name used in the DataFusion query and grouped result schema.
    pub fn result_name(&self) -> String {
        match self.truncation {
            None    => self.arrow_name.clone(),
            Some(t) => format!("{}_{}", self.arrow_name, t.alias_suffix()),
        }
    }
    /// Display name shown in the "Group" header section.
    pub fn display_name_for(&self, original_display: &str) -> String {
        match self.truncation {
            None    => original_display.to_string(),
            Some(t) => format!("{} ({})", original_display, t.display_name()),
        }
    }
}

/// Restricts which date truncation levels appear in the column ⋮ menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AvailableDateTruncations {
    Global(Vec<DateTruncation>),
    PerColumn(std::collections::HashMap<String, Vec<DateTruncation>>),
}

/// Cached inference result for date truncation options.
#[derive(Debug, Clone)]
pub struct DateTruncationOptions {
    pub available: Vec<DateTruncation>,
    /// True if the options were inferred assuming sorted data (may be inaccurate).
    pub is_estimate: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupByState {
    pub group_keys: Vec<DateGroupKey>,
    pub aggregations: Vec<(String, Vec<AggregateFunction>)>,
}

impl GroupByState {
    pub fn is_active(&self) -> bool {
        !self.group_keys.is_empty()
    }

    /// True if any group key (regardless of truncation) uses this source column.
    pub fn has_source_column(&self, arrow_name: &str) -> bool {
        self.group_keys.iter().any(|k| k.arrow_name == arrow_name)
    }

    /// True if a group key with exactly this arrow_name AND truncation exists.
    pub fn has_exact_key(&self, arrow_name: &str, truncation: Option<DateTruncation>) -> bool {
        self.group_keys.iter().any(|k| k.arrow_name == arrow_name && k.truncation == truncation)
    }

    /// All group keys whose source column is this arrow_name.
    pub fn keys_for_source<'a>(&'a self, arrow_name: &str) -> Vec<&'a DateGroupKey> {
        self.group_keys.iter().filter(|k| k.arrow_name == arrow_name).collect()
    }

    pub fn agg_fns_for(&self, arrow_name: &str) -> Option<&[AggregateFunction]> {
        self.aggregations
            .iter()
            .find(|(n, _)| n == arrow_name)
            .map(|(_, fns)| fns.as_slice())
    }

    pub fn set_agg_fns(&mut self, arrow_name: &str, fns: Vec<AggregateFunction>) {
        if let Some(entry) = self.aggregations.iter_mut().find(|(n, _)| n == arrow_name) {
            entry.1 = fns;
        } else {
            self.aggregations.push((arrow_name.to_string(), fns));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SortState {
    pub column: Option<usize>,
    pub direction: Option<SortDirection>,
}

#[cfg(test)]
mod col_tests {
    use super::*;
    use crate::columns::{normalize_columns, resolve_columns};

    #[test]
    fn test_column_input_deser() {
        let json = r#####"[
          {
            "display": "Financials",
            "headerStyle": {"bgColor": "#E8EAF6", "color": "#283593"},
            "children": [
              {
                "name": "price",
                "display": "Price",
                "initWidth": 120,
                "dataStyle": {
                  "numberFormat": {"type": "currency", "symbol": "$", "decimals": 2},
                  "align": "right"
                }
              }
            ]
          }
        ]"#####;
        let result: Result<Vec<ColumnInput>, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "deser failed: {:?}", result.err());
        let cols = result.unwrap();
        assert_eq!(cols[0].display.as_deref(), Some("Financials"));
    }

    #[test]
    fn test_resolve_columns() {
        let json = r#####"[
          {"display": "Descriptions", "children": [
            {"name": "id", "display": "ID", "initWidth": 80},
            {"name": "name", "display": "Product Name", "initWidth": 200}
          ]},
          {"display": "Financials", "children": [
            {"name": "price", "display": "Price", "initWidth": 120}
          ]}
        ]"#####;
        let inputs: Vec<ColumnInput> = serde_json::from_str(json).unwrap();
        let data_cols = vec!["id".to_string(), "name".to_string(), "price".to_string()];
        let normalized = normalize_columns(Some(&inputs), None, &data_cols).unwrap();
        let resolved = resolve_columns(&normalized, &data_cols, 150.0, 1.0);
        assert!(resolved.is_ok(), "resolve failed: {:?}", resolved.err());
        let r = resolved.unwrap();
        assert_eq!(r.max_depth, 2);
        assert_eq!(r.leaves.len(), 3);
        assert_eq!(r.leaves[0].display_name, "ID");
        assert_eq!(r.leaves[1].display_name, "Product Name");
        assert_eq!(r.leaves[2].display_name, "Price");
    }

    #[test]
    fn test_conditional_format_deser() {
        let json = r#####"[
          {"display": "Financials", "children": [
            {"name": "qty", "display": "Qty", "dataStyle": {
              "numberFormat": {"type": "integer"},
              "conditionalFormats": [
                {"type": "greaterThan", "value": 5000, "style": {"bgColor": "#C8E6C9", "color": "#1B5E20"}}
              ]
            }}
          ]}
        ]"#####;
        let result: Result<Vec<ColumnInput>, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "deser failed: {:?}", result.err());
    }
}
