use std::collections::HashMap;

const CSS_NAMED_COLORS: &[(&str, (u8, u8, u8))] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("red", (255, 0, 0)),
    ("green", (0, 128, 0)),
    ("blue", (0, 0, 255)),
    ("transparent", (0, 0, 0)),
    ("gray", (128, 128, 128)),
    ("grey", (128, 128, 128)),
    ("silver", (192, 192, 192)),
    ("maroon", (128, 0, 0)),
    ("purple", (128, 0, 128)),
    ("fuchsia", (255, 0, 255)),
    ("lime", (0, 255, 0)),
    ("olive", (128, 128, 0)),
    ("yellow", (255, 255, 0)),
    ("navy", (0, 0, 128)),
    ("teal", (0, 128, 128)),
    ("aqua", (0, 255, 255)),
    ("orange", (255, 165, 0)),
];

/// Parses a color string to [r, g, b, a] where each component is 0-255 (alpha is 0.0-1.0).
pub fn parse_to_rgba(color: &str) -> [f32; 4] {
    let color = color.trim().to_lowercase();

    if color == "transparent" {
        return [0.0, 0.0, 0.0, 0.0];
    }

    // Try hex
    if let Some(hex) = color.strip_prefix('#') {
        return parse_hex(hex);
    }

    // Try rgba()
    if let Some(inner) = color.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 4 {
            let r = parts[0].parse::<f32>().unwrap_or(0.0);
            let g = parts[1].parse::<f32>().unwrap_or(0.0);
            let b = parts[2].parse::<f32>().unwrap_or(0.0);
            let a = parts[3].parse::<f32>().unwrap_or(1.0);
            return [r, g, b, a];
        }
    }

    // Try rgb()
    if let Some(inner) = color.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<f32>().unwrap_or(0.0);
            let g = parts[1].parse::<f32>().unwrap_or(0.0);
            let b = parts[2].parse::<f32>().unwrap_or(0.0);
            return [r, g, b, 1.0];
        }
    }

    // Named colors
    for &(name, (r, g, b)) in CSS_NAMED_COLORS {
        if color == name {
            return [r as f32, g as f32, b as f32, 1.0];
        }
    }

    // Fallback
    [0.0, 0.0, 0.0, 1.0]
}

fn parse_hex(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');

    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = parse_hex_digit(hex.as_bytes()[0]);
            let g = parse_hex_digit(hex.as_bytes()[1]);
            let b = parse_hex_digit(hex.as_bytes()[2]);
            (r, g, b, 1.0f32)
        }
        4 => {
            let r = parse_hex_digit(hex.as_bytes()[0]);
            let g = parse_hex_digit(hex.as_bytes()[1]);
            let b = parse_hex_digit(hex.as_bytes()[2]);
            let a = parse_hex_digit(hex.as_bytes()[3]) as f32 / 255.0;
            (r, g, b, a)
        }
        6 => {
            let r = parse_hex_pair(&hex[0..2]);
            let g = parse_hex_pair(&hex[2..4]);
            let b = parse_hex_pair(&hex[4..6]);
            (r, g, b, 1.0f32)
        }
        8 => {
            let r = parse_hex_pair(&hex[0..2]);
            let g = parse_hex_pair(&hex[2..4]);
            let b = parse_hex_pair(&hex[4..6]);
            let a = parse_hex_pair(&hex[6..8]) as f32 / 255.0;
            (r, g, b, a)
        }
        _ => (0.0, 0.0, 0.0, 1.0),
    };

    [r, g, b, a]
}

fn parse_hex_digit(c: u8) -> f32 {
    match c {
        b'0'..=b'9' => (c - b'0') as f32 * 17.0,
        b'a'..=b'f' => (c - b'a' + 10) as f32 * 17.0,
        b'A'..=b'F' => (c - b'A' + 10) as f32 * 17.0,
        _ => 0.0,
    }
}

fn parse_hex_pair(s: &str) -> f32 {
    u8::from_str_radix(s, 16).unwrap_or(0) as f32
}

/// Formats [r, g, b, a] back to an rgba() CSS string.
pub fn rgba_to_string(rgba: [f32; 4]) -> String {
    if rgba[3] >= 1.0 {
        format!(
            "rgb({}, {}, {})",
            rgba[0].round() as u8,
            rgba[1].round() as u8,
            rgba[2].round() as u8
        )
    } else {
        format!(
            "rgba({}, {}, {}, {})",
            rgba[0].round() as u8,
            rgba[1].round() as u8,
            rgba[2].round() as u8,
            rgba[3]
        )
    }
}

/// Blends a color with a background. If the color has alpha < 1, it is composited.
pub fn blend(color: &str, background: &str) -> String {
    let [r, g, b, a] = parse_to_rgba(color);
    if a >= 1.0 {
        return color.to_string();
    }
    let [br, bg, bb, ba] = parse_to_rgba(background);
    let ao = a + ba * (1.0 - a);
    if ao == 0.0 {
        return "rgba(0, 0, 0, 0)".to_string();
    }
    let ro = (a * r + ba * br * (1.0 - a)) / ao;
    let go = (a * g + ba * bg * (1.0 - a)) / ao;
    let bo = (a * b + ba * bb * (1.0 - a)) / ao;
    rgba_to_string([ro, go, bo, ao])
}

pub fn with_alpha(color: &str, alpha: f32) -> String {
    let [r, g, b, _] = parse_to_rgba(color);
    rgba_to_string([r, g, b, alpha])
}

pub fn interpolate_colors(left: &str, right: &str, val: f32) -> String {
    if val <= 0.0 {
        return left.to_string();
    }
    if val >= 1.0 {
        return right.to_string();
    }

    let [lr, lg, lb, la] = parse_to_rgba(left);
    let [rr, rg, rb, ra] = parse_to_rgba(right);

    // Pre-multiplied alpha
    let lr = lr * la;
    let lg = lg * la;
    let lb = lb * la;
    let rr = rr * ra;
    let rg = rg * ra;
    let rb = rb * ra;

    let n = 1.0 - val;
    let a = la * n + ra * val;
    if a == 0.0 {
        return "rgba(0, 0, 0, 0)".to_string();
    }
    let r = ((lr * n + rr * val) / a).round() as u8;
    let g = ((lg * n + rg * val) / a).round() as u8;
    let b = ((lb * n + rb * val) / a).round() as u8;
    format!("rgba({}, {}, {}, {})", r, g, b, a)
}

pub fn get_luminance(color: &str) -> f32 {
    if color == "transparent" {
        return 0.0;
    }
    let f = |x: f32| -> f32 {
        let c = x / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b, _] = parse_to_rgba(color);
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

/// Simple blend cache for repeated blend operations.
pub struct BlendCache {
    cache: HashMap<(String, String), String>,
}

impl BlendCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn blend(&mut self, color: &str, background: &str) -> String {
        let key = (color.to_string(), background.to_string());
        if let Some(result) = self.cache.get(&key) {
            return result.clone();
        }
        let result = blend(color, background);
        self.cache.insert(key, result.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_to_rgba("#ff0000"), [255.0, 0.0, 0.0, 1.0]);
        assert_eq!(parse_to_rgba("#00ff00"), [0.0, 255.0, 0.0, 1.0]);
        assert_eq!(parse_to_rgba("#0000ff"), [0.0, 0.0, 255.0, 1.0]);
        assert_eq!(parse_to_rgba("#fff"), [255.0, 255.0, 255.0, 1.0]);
        assert_eq!(parse_to_rgba("#f00"), [255.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_parse_rgba() {
        assert_eq!(parse_to_rgba("rgba(255, 0, 0, 1)"), [255.0, 0.0, 0.0, 1.0]);
        assert_eq!(parse_to_rgba("rgba(0, 0, 0, 0.5)"), [0.0, 0.0, 0.0, 0.5]);
    }

    #[test]
    fn test_transparent() {
        assert_eq!(parse_to_rgba("transparent"), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_blend() {
        let result = blend("rgba(255, 0, 0, 0.5)", "rgb(255, 255, 255)");
        // Half red over white should be pinkish
        assert!(result.contains("rgba("));
    }
}
