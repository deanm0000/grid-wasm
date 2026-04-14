/// Default column width in pixels used when none is specified.
pub const DEFAULT_COLUMN_WIDTH: f64 = 150.0;

/// Minimum column width a user can resize to.
pub const MIN_COLUMN_WIDTH: f64 = 30.0;

/// Minimum width for aggregated child columns when splitting a parent.
pub const MIN_AGG_CHILD_COLUMN_WIDTH: f64 = 80.0;

/// Default header height in pixels.
pub const DEFAULT_HEADER_HEIGHT: f64 = 36.0;

/// Default row height in pixels.
pub const DEFAULT_ROW_HEIGHT: f64 = 34.0;

/// Default column swap animation duration in milliseconds.
pub const DEFAULT_SWAP_ANIMATION_DURATION_MS: f64 = 250.0;

/// Pixel distance mouse must move before a column drag activates.
pub const COL_DRAG_ACTIVATE_PX: f64 = 8.0;

/// Fraction of the neighbor column width the mouse must cross to trigger a forward swap.
pub const COL_SWAP_THRESHOLD_FORWARD: f64 = 0.1;

/// Fraction of the neighbor column width used to block backward swaps (prevents oscillation).
pub const COL_SWAP_THRESHOLD_BACKWARD: f64 = 0.9;

/// Fallback column width used when a layout entry is unexpectedly missing.
pub const FALLBACK_COLUMN_WIDTH: f64 = 100.0;

/// Number of rows to jump on PageUp / PageDown.
pub const PAGE_SCROLL_ROWS: i32 = 20;
