use serde::{Deserialize, Serialize};

/// Default corner rounding radius in pixels used when the theme does not specify one.
const DEFAULT_ROUNDING_RADIUS: f64 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub accent_color: String,
    pub accent_fg: String,
    pub accent_light: String,
    pub text_dark: String,
    pub text_medium: String,
    pub text_light: String,
    pub text_bubble: String,
    pub bg_icon_header: String,
    pub fg_icon_header: String,
    pub text_header: String,
    pub text_header_selected: String,
    pub bg_cell: String,
    pub bg_cell_medium: String,
    pub bg_header: String,
    pub bg_header_has_focus: String,
    pub bg_header_hovered: String,
    pub bg_bubble: String,
    pub bg_bubble_selected: String,
    pub bg_search_result: String,
    pub border_color: String,
    pub drilldown_border: String,
    pub link_color: String,
    pub cell_horizontal_padding: f64,
    pub cell_vertical_padding: f64,
    pub header_font_style: String,
    pub header_icon_size: f64,
    pub base_font_style: String,
    pub marker_font_style: String,
    pub font_family: String,
    pub editor_font_size: String,
    pub line_height: f64,
    pub checkbox_max_size: f64,

    // Optional overrides
    pub resize_indicator_color: Option<String>,
    pub horizontal_border_color: Option<String>,
    pub header_bottom_border_color: Option<String>,
    pub rounding_radius: Option<f64>,
    pub text_group_header: Option<String>,
    pub bg_group_header: Option<String>,
    pub bg_group_header_hovered: Option<String>,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            accent_color: "#4F5DFF".to_string(),
            accent_fg: "#FFFFFF".to_string(),
            accent_light: "rgba(62, 116, 253, 0.1)".to_string(),
            text_dark: "#313139".to_string(),
            text_medium: "#737383".to_string(),
            text_light: "#B2B2C0".to_string(),
            text_bubble: "#313139".to_string(),
            bg_icon_header: "#737383".to_string(),
            fg_icon_header: "#FFFFFF".to_string(),
            text_header: "#313139".to_string(),
            text_header_selected: "#FFFFFF".to_string(),
            bg_cell: "#FFFFFF".to_string(),
            bg_cell_medium: "#FAFAFB".to_string(),
            bg_header: "#F7F7F8".to_string(),
            bg_header_has_focus: "#E9E9EB".to_string(),
            bg_header_hovered: "#EFEFF1".to_string(),
            bg_bubble: "#EDEDF3".to_string(),
            bg_bubble_selected: "#FFFFFF".to_string(),
            bg_search_result: "#fff9e3".to_string(),
            border_color: "rgba(115, 116, 131, 0.16)".to_string(),
            drilldown_border: "rgba(0, 0, 0, 0)".to_string(),
            link_color: "#353fb5".to_string(),
            cell_horizontal_padding: 8.0,
            cell_vertical_padding: 3.0,
            header_icon_size: 18.0,
            header_font_style: "600 13px".to_string(),
            base_font_style: "13px".to_string(),
            marker_font_style: "9px".to_string(),
            font_family: "Inter, Roboto, -apple-system, BlinkMacSystemFont, avenir next, avenir, segoe ui, helvetica neue, helvetica, Ubuntu, noto, arial, sans-serif".to_string(),
            editor_font_size: "13px".to_string(),
            line_height: 1.4,
            checkbox_max_size: 18.0,
            resize_indicator_color: None,
            horizontal_border_color: None,
            header_bottom_border_color: None,
            rounding_radius: None,
            text_group_header: Some("#313139BB".to_string()),
            bg_group_header: None,
            bg_group_header_hovered: None,
        }
    }

    /// Full font string for base cell text, e.g. "13px Inter, sans-serif"
    pub fn base_font_full(&self) -> String {
        format!("{} {}", self.base_font_style, self.font_family)
    }

    /// Full font string for headers
    pub fn header_font_full(&self) -> String {
        format!("{} {}", self.header_font_style, self.font_family)
    }

    pub fn horizontal_border_color(&self) -> &str {
        self.horizontal_border_color.as_deref().unwrap_or(&self.border_color)
    }

    pub fn text_group_header(&self) -> &str {
        self.text_group_header.as_deref().unwrap_or(&self.text_header)
    }

    pub fn bg_group_header(&self) -> &str {
        self.bg_group_header.as_deref().unwrap_or(&self.bg_header)
    }

    pub fn rounding_radius(&self) -> f64 {
        self.rounding_radius.unwrap_or(DEFAULT_ROUNDING_RADIUS)
    }

    pub fn merge_with(&self, overrides: &ThemeOverride) -> Theme {
        let mut t = self.clone();
        if let Some(ref v) = overrides.accent_color { t.accent_color = v.clone(); }
        if let Some(ref v) = overrides.bg_cell { t.bg_cell = v.clone(); }
        if let Some(ref v) = overrides.bg_header { t.bg_header = v.clone(); }
        if let Some(ref v) = overrides.text_dark { t.text_dark = v.clone(); }
        if let Some(ref v) = overrides.text_medium { t.text_medium = v.clone(); }
        if let Some(ref v) = overrides.text_light { t.text_light = v.clone(); }
        if let Some(ref v) = overrides.border_color { t.border_color = v.clone(); }
        t
    }
}

/// Partial theme for overrides on columns/rows/cells.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeOverride {
    pub accent_color: Option<String>,
    pub bg_cell: Option<String>,
    pub bg_header: Option<String>,
    pub text_dark: Option<String>,
    pub text_medium: Option<String>,
    pub text_light: Option<String>,
    pub border_color: Option<String>,
}
