use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::CanvasRenderingContext2d;

/// Thin wrapper around the CanvasRenderingContext2d for ergonomic Rust usage.
pub struct CanvasCtx {
    ctx: CanvasRenderingContext2d,
    text_cache: HashMap<String, f64>,
    cache_hits: usize,
    cache_misses: usize,
}

impl CanvasCtx {
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Self {
            ctx,
            text_cache: HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    // --- State ---

    pub fn save(&self) {
        self.ctx.save();
    }

    pub fn restore(&self) {
        self.ctx.restore();
    }

    // --- Fill / Stroke ---

    pub fn set_fill_style(&self, color: &str) {
        self.ctx.set_fill_style_str(color);
    }

    pub fn set_stroke_style(&self, color: &str) {
        self.ctx.set_stroke_style_str(color);
    }

    pub fn set_global_alpha(&self, alpha: f64) {
        self.ctx.set_global_alpha(alpha);
    }

    pub fn set_line_width(&self, width: f64) {
        self.ctx.set_line_width(width);
    }

    pub fn set_line_cap(&self, cap: &str) {
        self.ctx.set_line_cap(cap);
    }

    pub fn set_line_join(&self, join: &str) {
        self.ctx.set_line_join(join);
    }

    pub fn set_line_dash(&self, segments: &[f64]) {
        let arr = js_sys::Array::new();
        for &s in segments {
            arr.push(&JsValue::from_f64(s));
        }
        let _ = self.ctx.set_line_dash(&arr);
    }

    // --- Text ---

    pub fn set_font(&self, font: &str) {
        self.ctx.set_font(font);
    }

    pub fn set_text_align(&self, align: &str) {
        self.ctx.set_text_align(align);
    }

    pub fn set_text_baseline(&self, baseline: &str) {
        self.ctx.set_text_baseline(baseline);
    }

    pub fn fill_text(&self, text: &str, x: f64, y: f64) -> Result<(), JsValue> {
        self.ctx.fill_text(text, x, y)
    }

    pub fn measure_text_width(&mut self, text: &str, font: &str) -> f64 {
        let key = format!("{}\x00{}", font, text);
        if let Some(&w) = self.text_cache.get(&key) {
            self.cache_hits += 1;
            return w;
        }
        self.cache_misses += 1;
        self.ctx.set_font(font);
        let metrics = self.ctx.measure_text(text).unwrap_or_else(|_| {
            // fallback: create a dummy text metrics
            self.ctx.measure_text("").unwrap()
        });
        let w = metrics.width();
        if self.text_cache.len() < 5000 {
            self.text_cache.insert(key, w);
        } else if self.cache_misses > 1000 {
            // Periodic cache clear
            self.text_cache.clear();
            self.cache_hits = 0;
            self.cache_misses = 0;
        }
        w
    }

    pub fn measure_text(&mut self, text: &str, font: &str) -> web_sys::TextMetrics {
        self.ctx.set_font(font);
        self.ctx.measure_text(text).unwrap_or_else(|_| {
            self.ctx.set_font(font);
            self.ctx.measure_text("").unwrap()
        })
    }

    pub fn clear_text_cache(&mut self) {
        self.text_cache.clear();
    }

    // --- Paths ---

    pub fn begin_path(&self) {
        self.ctx.begin_path();
    }

    pub fn close_path(&self) {
        self.ctx.close_path();
    }

    pub fn move_to(&self, x: f64, y: f64) {
        self.ctx.move_to(x, y);
    }

    pub fn line_to(&self, x: f64, y: f64) {
        self.ctx.line_to(x, y);
    }

    pub fn arc(&self, x: f64, y: f64, radius: f64, start: f64, end: f64) -> Result<(), JsValue> {
        self.ctx.arc(x, y, radius, start, end)
    }

    pub fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.fill_rect(x, y, w, h);
    }

    pub fn stroke_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.stroke_rect(x, y, w, h);
    }

    pub fn fill(&self) {
        self.ctx.fill();
    }

    pub fn fill_with_winding(_winding: &str) {}

    pub fn stroke(&self) {
        self.ctx.stroke();
    }

    // --- Clipping ---

    pub fn clip(&self) {
        self.ctx.clip();
    }

    pub fn clip_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.begin_path();
        self.ctx.rect(x, y, w, h);
        self.ctx.clip();
    }

    // --- Transform ---

    pub fn clear_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.ctx.clear_rect(x, y, w, h);
    }

    pub fn set_direction(&self, _dir: &str) {
        // web-sys CanvasRenderingContext2d doesn't expose set_direction_str in all versions
        // Direction is handled at the fillText level instead
    }

    // --- Direct access for when the wrapper doesn't suffice ---
    pub fn raw(&self) -> &CanvasRenderingContext2d {
        &self.ctx
    }
}

use wasm_bindgen::JsValue;
