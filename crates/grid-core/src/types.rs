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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GridColumnIcon {
    HeaderString,
    HeaderNumber,
    HeaderBoolean,
    HeaderUri,
    HeaderMarkdown,
    HeaderDate,
    HeaderTime,
    HeaderEmail,
    HeaderCode,
    HeaderRowID,
    HeaderImage,
    HeaderEmoji,
    HeaderPhone,
    HeaderAudioUri,
    HeaderVideoUri,
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

/// A compact representation of a set of indices (rows or columns).
/// Uses sorted non-overlapping ranges for memory efficiency.
#[derive(Debug, Clone, Default)]
pub struct CompactSelection {
    ranges: Vec<(u32, u32)>, // (inclusive_start, inclusive_end)
}

impl CompactSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_indices(indices: &[i32]) -> Self {
        let mut sorted: Vec<u32> = indices.iter().filter(|&&i| i >= 0).map(|&i| i as u32).collect();
        sorted.sort_unstable();
        sorted.dedup();

        let mut ranges = Vec::new();
        let mut start = match sorted.first() {
            Some(&s) => s,
            None => return Self { ranges },
        };
        let mut end = start;

        for &idx in sorted.iter().skip(1) {
            if idx == end + 1 {
                end = idx;
            } else {
                ranges.push((start, end));
                start = idx;
                end = idx;
            }
        }
        ranges.push((start, end));

        Self { ranges }
    }

    pub fn has_index(&self, index: i32) -> bool {
        if index < 0 {
            return false;
        }
        let idx = index as u32;
        self.ranges
            .binary_search_by(|(start, end)| {
                if idx < *start {
                    std::cmp::Ordering::Greater
                } else if idx > *end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GridColumnMenuIcon {
    Triangle,
    Dots,
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
}

fn default_two() -> u32 { 2 }
fn default_one() -> u32 { 1 }
fn default_dollar() -> String { "$".to_string() }
fn default_date_format() -> String { "%Y-%m-%d".to_string() }

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataStyle {
    pub font: Option<String>,
    pub color: Option<String>,
    pub bg_color: Option<String>,
    pub align: Option<ContentAlign>,
    pub number_format: Option<NumberFormat>,
    pub conditional_formats: Option<Vec<ConditionalRule>>,
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
}

fn default_true() -> bool { true }

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
