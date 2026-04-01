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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentSelection {
    pub cell: Item,
    pub range: Rectangle,
}

impl Default for GridSelection {
    fn default() -> Self {
        Self {
            current: None,
            columns: Vec::new(),
            rows: Vec::new(),
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
