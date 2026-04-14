import wasmInit, { DataGrid as WasmDataGridInternal } from './wasm/grid_core.js';
import Coloris from '@melloware/coloris';
import '@melloware/coloris/dist/coloris.css';

// ─── Layout constants ───────────────────────────────────────────────────────
/** Default scrollbar track width/height in pixels. */
const DEFAULT_SCROLLBAR_WIDTH = 12;
/** Minimum scrollbar thumb size in pixels to keep it clickable. */
const MIN_SCROLLBAR_THUMB_SIZE = 20;
/** z-index for the scrollbar track elements. */
const SCROLLBAR_Z_INDEX = '9999';
/** Default opacity of the scrollbar thumb. */
const SCROLLBAR_THUMB_OPACITY = 0.25;
/** Opacity of the scrollbar thumb on hover. */
const SCROLLBAR_THUMB_HOVER_OPACITY = 0.4;
/** Opacity of the scrollbar track background. */
const SCROLLBAR_TRACK_OPACITY = 0.95;
/** Inset of the thumb inside the track on each side in pixels. */
const SCROLLBAR_THUMB_INSET = 2;

// ─── Interaction constants ───────────────────────────────────────────────────
/** Pixel threshold a drag must exceed before it is considered a real drag (JS side). */
const DRAG_ACTIVATE_THRESHOLD_PX = 4;

// ─── Menu UI constants ────────────────────────────────────────────────────────
/** z-index for header menus (above scrollbars). */
const MENU_Z_INDEX = '10001';
/** z-index for the column drag chip. */
const COL_DRAG_CHIP_Z_INDEX = '10000';
/** Fallback menu width estimate before the element is measured. */
const MENU_FALLBACK_WIDTH = 190;
/** Fallback menu height estimate before the element is measured. */
const MENU_FALLBACK_HEIGHT = 200;
/** Vertical gap between the click position and the top of the menu. */
const MENU_Y_OFFSET = 4;
/** Minimum distance from viewport edges the menu must maintain. */
const MENU_VIEWPORT_MARGIN = 8;
/** Horizontal gap between a parent menu item and its submenu. */
const SUBMENU_X_GAP = 4;
/** Maximum number of decimal places the format +/− spinner can reach. */
const MAX_DECIMAL_PLACES = 6;
/** X offset of the drag chip from the cursor. */
const COL_DRAG_CHIP_X_OFFSET = 12;
/** Y offset of the drag chip above the cursor. */
const COL_DRAG_CHIP_Y_OFFSET = 14;
/** Minimum number of sibling aggregations required to show the Remove option. */
const MIN_SIBLING_AGGS_TO_REMOVE = 2;

export interface GridColumn {
  title: string;
  width: number;
  group?: string;
  icon?: string;
  id?: string;
}

export interface TextCell {
  kind: 'text';
  data: string;
  displayData?: string;
  contentAlign?: 'left' | 'center' | 'right';
}

export interface NumberCell {
  kind: 'number';
  data?: number;
  displayData?: string;
  contentAlign?: 'left' | 'center' | 'right';
}

export interface LoadingCell {
  kind: 'loading';
  skeletonWidth?: number;
}

export type GridCell = TextCell | NumberCell | LoadingCell;

export interface Theme {
  accent_color?: string;
  accent_fg?: string;
  accent_light?: string;
  text_dark?: string;
  text_medium?: string;
  text_light?: string;
  text_bubble?: string;
  bg_icon_header?: string;
  fg_icon_header?: string;
  text_header?: string;
  text_header_selected?: string;
  bg_cell?: string;
  bg_cell_medium?: string;
  bg_header?: string;
  bg_header_has_focus?: string;
  bg_header_hovered?: string;
  bg_bubble?: string;
  bg_bubble_selected?: string;
  bg_search_result?: string;
  border_color?: string;
  drilldown_border?: string;
  link_color?: string;
  cell_horizontal_padding?: number;
  cell_vertical_padding?: number;
  header_font_style?: string;
  header_icon_size?: number;
  base_font_style?: string;
  marker_font_style?: string;
  font_family?: string;
  editor_font_size?: string;
  line_height?: number;
  checkbox_max_size?: number;
  [key: string]: any;
}

export interface SchemaField {
  name: string;
  type: string;
}

export interface SortState {
  column: number;
  direction: 'asc' | 'desc';
}

export interface HeaderStyle {
  font?: string;
  color?: string;
  bgColor?: string;
}

export interface CellStyleOverride {
  color?: string;
  bgColor?: string;
  font?: string;
}

export type NumberFormat =
  | { type: 'accounting'; decimals?: number }
  | { type: 'currency'; symbol?: string; decimals?: number }
  | { type: 'percent'; decimals?: number }
  | { type: 'decimal'; decimals?: number }
  | { type: 'integer' }
  | { type: 'date'; format?: string }
  | { type: 'dateTime'; format?: string };

export type ConditionalRule =
  | { type: 'greaterThan'; value: number; style: CellStyleOverride }
  | { type: 'lessThan'; value: number; style: CellStyleOverride }
  | { type: 'equal'; value: number; style: CellStyleOverride }
  | { type: 'between'; min: number; max: number; style: CellStyleOverride }
  | { type: 'contains'; value: string; style: CellStyleOverride }
  | { type: 'isNull'; style: CellStyleOverride }
  | { type: 'isNotNull'; style: CellStyleOverride }
  | { type: 'percentile'; low?: number; high?: number; lowStyle: CellStyleOverride; midStyle?: CellStyleOverride; highStyle: CellStyleOverride };

export interface DataStyle extends HeaderStyle {
  align?: 'left' | 'center' | 'right';
  numberFormat?: NumberFormat;
  conditionalFormats?: ConditionalRule[];
}

export type AggFuncName = 'count' | 'sum' | 'min' | 'max' | 'mean';

export type DateTruncationLevel =
  | 'year' | 'quarter' | 'month' | 'week' | 'day'
  | 'hour' | 'minute' | 'second'
  | 'millisecond' | 'microsecond' | 'nanosecond';

export interface ColumnInput {
  name?: string;
  display?: string;
  initWidth?: number;
  isResizable?: boolean;
  headerStyle?: HeaderStyle;
  dataStyle?: DataStyle;
  children?: ColumnInput[];
  /** Initial aggregation function(s) when this column enters grouped mode. */
  aggFunc?: AggFuncName[];
  /** Relative ordering as a group-by key on load (0 = leftmost). */
  groupBy?: number;
  /** For date/datetime columns: truncation level when used as a group-by key. */
  groupByTruncation?: DateTruncationLevel;
}

export type DataSource =
  | { kind: 'arrow-ipc'; bytes: Uint8Array }
  | { kind: 'arrow-stream-url'; url: string }
  | { kind: 'objects'; data: Record<string, any>[] }
  | { kind: 'callback'; rows: number; columns: GridColumn[]; getCellContent: (col: number, row: number) => GridCell };

export interface DataGridOptions {
  data: DataSource;
  columns?: ColumnInput[];
  columnOverrides?: ColumnInput[];
  headerHeight?: number;
  rowHeight?: number;
  freezeColumns?: number;
  freezeTrailingRows?: number;
  theme?: Theme;
  initialSortState?: SortState;
  swapAnimationDuration?: number;
  showScrollbars?: boolean;
  scrollbarWidth?: number;
  /** Arrow names of columns that may be used as group-by keys.
   *  If absent, all columns are allowed. */
  allowableGroupBy?: string[];
  /** Arrow names of columns that are always group-by keys (user cannot remove them). */
  mandatoryGroupBy?: string[];
  /** Restricts which aggregate functions appear in the column ⋮ menu. */
  availableAggregateFunctions?: AggFuncName[] | Record<string, AggFuncName[]>;
  /** Restricts which date truncation levels appear in the column ⋮ menu.
   *  A string array applies globally; an object applies per column by arrow_name. */
  availableDateTruncations?: DateTruncationLevel[] | Record<string, DateTruncationLevel[]>;
}

/** Walk a ColumnInput tree and collect group-by keys sorted by their `groupBy` value. */
function collectGroupByKeys(inputs: ColumnInput[]): Array<{ name: string; truncation?: string }> {
  const found: Array<{ order: number; name: string; truncation?: string }> = [];
  function walk(nodes: ColumnInput[]) {
    for (const node of nodes) {
      if (node.children) {
        walk(node.children);
      } else if (node.name && node.groupBy !== undefined) {
        found.push({ order: node.groupBy, name: node.name, truncation: node.groupByTruncation });
      }
    }
  }
  walk(inputs);
  found.sort((a, b) => a.order - b.order);
  return found.map(f => ({ name: f.name, truncation: f.truncation }));
}

let wasmReady: Promise<any> | null = null;

function ensureWasmInit(): Promise<any> {
  if (!wasmReady) {
    wasmReady = wasmInit();
  }
  return wasmReady;
}

export class WasmDataGrid {
  private grid: InstanceType<typeof WasmDataGridInternal> | null = null;
  private canvas: HTMLCanvasElement;
  private observer: ResizeObserver | null = null;
  private options: DataGridOptions;
  private sortPending = false;
  private isResizing = false;
  private isDragging = false;
  private isColDragging = false;
  private dragMoved = false;
  private mouseDownX = 0;
  private mouseDownY = 0;
  private mouseDownShift = false;
  private mouseDownCtrl = false;
  private colDragChip: HTMLDivElement | null = null;
  private wasColDragging = false;
  private animFrameId: number | null = null;
  private prevCursor = 'default';
  private cfPanelEl: HTMLDivElement | null = null;
  private colorisInitialized = false;
  private headerMenuEl: HTMLDivElement | null = null;
  private headerMenuCloseHandler: ((e: MouseEvent) => void) | null = null;
  private menuStyle: HTMLStyleElement | null = null;
  // Scrollbar elements
  private vScrollbar: HTMLDivElement | null = null;
  private vThumb: HTMLDivElement | null = null;
  private hScrollbar: HTMLDivElement | null = null;
  private hThumb: HTMLDivElement | null = null;
  private scrollCorner: HTMLDivElement | null = null;
  private scrollbarWidth = DEFAULT_SCROLLBAR_WIDTH;
  private vThumbDragging = false;
  private vThumbStartY = 0;
  private vThumbStartOffset = 0;
  private hThumbDragging = false;
  private hThumbStartX = 0;
  private hThumbStartTranslate = 0;

  constructor(canvas: HTMLCanvasElement, options: DataGridOptions) {
    this.canvas = canvas;
    this.options = options;
  }

  async init(): Promise<void> {
    await ensureWasmInit();
    this.grid = new WasmDataGridInternal(this.canvas);
    await this.configure();
    this.attachEvents();
    this.grid.render();
  }

  private async configure(): Promise<void> {
    if (!this.grid) return;

    const {
      data, columns, columnOverrides,
      headerHeight, rowHeight, freezeColumns, freezeTrailingRows,
      theme, initialSortState, swapAnimationDuration, showScrollbars, scrollbarWidth,
      allowableGroupBy, mandatoryGroupBy, availableAggregateFunctions, availableDateTruncations,
    } = this.options;

    const sbw = scrollbarWidth ?? DEFAULT_SCROLLBAR_WIDTH;
    this.scrollbarWidth = sbw;

    if (columns && columnOverrides) {
      throw new Error("Cannot specify both 'columns' and 'columnOverrides'");
    }

    // Set layout params before data so configure_columns uses correct header height
    if (headerHeight !== undefined) this.grid.set_header_height(headerHeight);
    if (rowHeight !== undefined) this.grid.set_row_height(rowHeight);
    if (freezeColumns !== undefined) this.grid.set_freeze_columns(freezeColumns);
    if (freezeTrailingRows !== undefined) this.grid.set_freeze_trailing_rows(freezeTrailingRows);
    if (swapAnimationDuration !== undefined) this.grid.set_swap_animation_duration(swapAnimationDuration);
    if (theme) this.grid.set_theme(theme);

    // Group-by constraints — set before column/data loading so they're active
    // when enter_group_by is triggered
    if (allowableGroupBy)           this.grid.set_allowable_group_by(JSON.stringify(allowableGroupBy));
    if (mandatoryGroupBy)           this.grid.set_mandatory_group_by(JSON.stringify(mandatoryGroupBy));
    if (availableAggregateFunctions)
      this.grid.set_available_aggregate_functions(JSON.stringify(availableAggregateFunctions));
    if (availableDateTruncations)
      this.grid.set_available_date_truncations(JSON.stringify(availableDateTruncations));

    if (columns) {
      this.grid.set_column_input(JSON.stringify(columns));
    } else if (columnOverrides) {
      this.grid.set_column_overrides(JSON.stringify(columnOverrides));
    }

    switch (data.kind) {
      case 'arrow-ipc': {
        this.grid.set_data_ipc(data.bytes);
        break;
      }
      case 'arrow-stream-url': {
        const response = await fetch(data.url);
        const bytes = new Uint8Array(await response.arrayBuffer());
        this.grid.set_data_ipc(bytes);
        break;
      }
      case 'objects': {
        const jsonStr = JSON.stringify(data.data);
        this.grid.set_data_objects(jsonStr);
        break;
      }
      case 'callback': {
        this.grid.set_rows(data.rows);
        this.grid.set_columns(data.columns);
        this.grid.set_cell_callback(data.getCellContent);
        break;
      }
    }

    // Auto-enter grouped mode if any columns have `groupBy` set
    const initGroupKeys = collectGroupByKeys(columns ?? columnOverrides ?? []);
    if (initGroupKeys.length > 0) {
      for (const key of initGroupKeys) {
        await this.grid.toggle_group_key_truncated(key.name, key.truncation ?? 'null');
      }
    }

    if (initialSortState) {
      this.grid.set_sort_state(initialSortState.column, initialSortState.direction);
      await this.grid.apply_sort();
    }

    this.updateSize();

    if (showScrollbars !== false) {
      this.initScrollbars();
    }
  }

  private updateSize(): void {
    if (!this.grid) return;
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    // Use the full CSS size for the canvas so getBoundingClientRect().width always
    // equals canvas.width / dpr. Scrollbars overlay on top of the grid.
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.grid.set_size(rect.width * dpr, rect.height * dpr);
    this.positionScrollbars();
  }

  private renderAndScroll(): void {
    this.grid?.render();
    this.updateScrollbars();
  }

  private attachEvents(): void {
    this.observer = new ResizeObserver(() => {
      this.updateSize();
      this.grid?.render();
      this.updateScrollbars();
    });
    this.observer.observe(this.canvas);

    this.canvas.addEventListener('mousedown', (e) => {
      if (!this.grid || e.button !== 0) return;
      const rect = this.canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = (e.clientX - rect.left) * dpr;
      const y = (e.clientY - rect.top) * dpr;

      this.mouseDownX = x;
      this.mouseDownY = y;
      this.mouseDownShift = e.shiftKey;
      this.mouseDownCtrl = e.ctrlKey || e.metaKey;
      this.dragMoved = false;

      const result = this.grid.on_mouse_down(x, y);
      if (result === 'resize') {
        this.isResizing = true;
        e.preventDefault();
      } else if (result === 'header-menu') {
        const colIdx = this.grid.get_last_menu_col();
        if (colIdx >= 0) {
          const ctx = JSON.parse(this.grid.get_column_menu_context(colIdx));
          this.showHeaderMenu(e.clientX, e.clientY, ctx);
        }
        e.preventDefault();
        e.stopPropagation();
      } else if (result === 'span-menu') {
        const spanCtx = JSON.parse(this.grid.get_span_menu_context());
        if (spanCtx) {
          this.showSpanMenu(e.clientX, e.clientY, spanCtx);
        }
        e.preventDefault();
        e.stopPropagation();
      } else if (result === 'expand-toggle') {
        const row = this.grid.get_last_expand_row();
        if (row >= 0) {
          e.preventDefault();
          this.grid.toggle_row_expand(row).then(() => {
            this.grid!.render();
            this.updateScrollbars();
          }).catch((e: unknown) => console.error('expand failed:', e));
        }
      } else if (result === 'col-drag') {
        this.isColDragging = true;
        e.preventDefault();
      } else if (result === 'drag') {
        this.isDragging = true;
      }
    });

    // click handles sort triangles (when !dragMoved) and shift/ctrl selection
    this.canvas.addEventListener('click', async (e) => {
      if (!this.grid || this.isResizing || this.dragMoved || this.isColDragging) return;
      if (this.wasColDragging) { this.wasColDragging = false; return; }

      const rect = this.canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = (e.clientX - rect.left) * dpr;
      const y = (e.clientY - rect.top) * dpr;

      // on_click handles: sort triangles + shift/ctrl modifiers
      // Plain cell selection was already committed in mouseup via on_drag_end
      const prevSort = this.grid.get_sort_state();
      this.grid.on_click(x, y, this.mouseDownShift, this.mouseDownCtrl);
      const nextSort = this.grid.get_sort_state();

      if (prevSort !== nextSort && !this.sortPending) {
        this.sortPending = true;
        try {
          await this.grid.apply_sort();
        } finally {
          this.sortPending = false;
        }
      }
      this.grid.render(); this.updateScrollbars();
    });

    this.canvas.addEventListener('mousemove', (e) => {
      if (!this.grid) return;
      const rect = this.canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = (e.clientX - rect.left) * dpr;
      const y = (e.clientY - rect.top) * dpr;

      if (this.isColDragging) {
        const cursor = this.grid.on_mouse_move(x, y);
        this.canvas.style.cursor = cursor;

        if (this.grid.is_col_drag_active()) {
          this.showColDragChip(e.clientX, e.clientY);
        }

        if (cursor === 'col-swap') {
          this.startAnimLoop();
        } else {
          this.grid.render();
        }
        return;
      }

      if (this.isDragging) {
        const dx = x - this.mouseDownX;
        const dy = y - this.mouseDownY;
        if (!this.dragMoved && (Math.abs(dx) > DRAG_ACTIVATE_THRESHOLD_PX || Math.abs(dy) > DRAG_ACTIVATE_THRESHOLD_PX)) {
          this.dragMoved = true;
        }
        if (this.dragMoved) {
          this.grid.on_drag_update(x, y);
          this.grid.render();
        }
        return;
      }

      if (this.isResizing) {
        this.grid.on_mouse_move(x, y);
        this.grid.render();
        return;
      }

      const cursor = this.grid.on_mouse_move(x, y);
      this.canvas.style.cursor = cursor;

      this.prevCursor = cursor;
    });

    window.addEventListener('mouseup', (e) => {
      if (!this.grid || e.button !== 0) return;
      const rect = this.canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = (e.clientX - rect.left) * dpr;
      const y = (e.clientY - rect.top) * dpr;

      if (this.isColDragging) {
        this.grid.on_col_drag_end();
        this.isColDragging = false;
        this.wasColDragging = true;
        this.hideColDragChip();
        this.canvas.style.cursor = 'default';
        this.grid.render(); this.updateScrollbars();
        return;
      }

      if (this.isDragging) {
        if (this.dragMoved) {
          this.grid.on_drag_end(x, y, true);
        } else if (!this.mouseDownShift && !this.mouseDownCtrl) {
          this.grid.on_drag_end(x, y, false);
        }
        this.isDragging = false;
        this.grid.render(); this.updateScrollbars();
        return;
      }

      if (this.isResizing) {
        this.grid.on_mouse_up(x, y);
        this.isResizing = false;
        this.canvas.style.cursor = 'default';
        this.grid.render(); this.updateScrollbars();
      }
    });

    this.canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      if (!this.grid) return;
      this.grid.on_scroll(e.deltaX, e.deltaY);
      this.grid.render();
      this.updateScrollbars();
    }, { passive: false });

    this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());

    this.canvas.setAttribute('tabindex', '0');
    this.canvas.addEventListener('keydown', (e) => {
      if (!this.grid) return;

      // Ctrl+C / Cmd+C — copy selection to clipboard
      if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
        const tsv = this.grid.get_selected_cells_tsv();
        if (tsv) {
          navigator.clipboard.writeText(tsv).catch(() => {
            const ta = document.createElement('textarea');
            ta.value = tsv;
            ta.style.position = 'fixed';
            ta.style.opacity = '0';
            document.body.appendChild(ta);
            ta.select();
            document.execCommand('copy');
            document.body.removeChild(ta);
          });
        }
        e.preventDefault();
        return;
      }

      this.grid.on_key_down(e.key, e.shiftKey, e.ctrlKey);
      this.grid.render();
      this.updateScrollbars();
      e.preventDefault();
    });

    this.canvas.addEventListener('focus', () => {
      this.grid?.set_focused(true);
      this.grid?.render();
    });

    this.canvas.addEventListener('blur', () => {
      this.grid?.set_focused(false);
      this.grid?.render();
    });
  }

  // --- Scrollbar ---

  private initScrollbars(): void {
    const sbw = this.scrollbarWidth;

    const mkDiv = (cls: string) => {
      const el = document.createElement('div');
      el.className = cls;
      el.style.position = 'fixed';
      el.style.zIndex = SCROLLBAR_Z_INDEX;
      el.style.boxSizing = 'border-box';
      return el;
    };

    const track = (el: HTMLDivElement, bg: string) => {
      el.style.background = bg;
      el.style.overflow = 'hidden';
    };

    const thumb = (el: HTMLDivElement) => {
      el.style.position = 'absolute';
      el.style.background = `rgba(0,0,0,${SCROLLBAR_THUMB_OPACITY})`;
      el.style.borderRadius = `${sbw / 2}px`;
      el.style.cursor = 'pointer';
      el.style.transition = 'background 0.15s';
      el.addEventListener('mouseover', () => el.style.background = `rgba(0,0,0,${SCROLLBAR_THUMB_HOVER_OPACITY})`);
      el.addEventListener('mouseout', () => el.style.background = `rgba(0,0,0,${SCROLLBAR_THUMB_OPACITY})`);
    };

    this.vScrollbar = mkDiv('grid-vscrollbar');
    this.vScrollbar.style.width = `${sbw}px`;
    track(this.vScrollbar, `rgba(240,240,240,${SCROLLBAR_TRACK_OPACITY})`);
    this.vThumb = mkDiv('grid-vthumb');
    this.vThumb.style.width = `calc(100% - ${SCROLLBAR_THUMB_INSET * 2}px)`;
    this.vThumb.style.left = `${SCROLLBAR_THUMB_INSET}px`;
    thumb(this.vThumb);
    this.vScrollbar.appendChild(this.vThumb);

    this.hScrollbar = mkDiv('grid-hscrollbar');
    this.hScrollbar.style.height = `${sbw}px`;
    track(this.hScrollbar, `rgba(240,240,240,${SCROLLBAR_TRACK_OPACITY})`);
    this.hThumb = mkDiv('grid-hthumb');
    this.hThumb.style.height = `calc(100% - ${SCROLLBAR_THUMB_INSET * 2}px)`;
    this.hThumb.style.top = `${SCROLLBAR_THUMB_INSET}px`;
    thumb(this.hThumb);
    this.hScrollbar.appendChild(this.hThumb);

    this.scrollCorner = mkDiv('grid-scrollcorner');
    this.scrollCorner.style.width = `${sbw}px`;
    this.scrollCorner.style.height = `${sbw}px`;
    this.scrollCorner.style.background = `rgba(240,240,240,${SCROLLBAR_TRACK_OPACITY})`;

    document.body.appendChild(this.vScrollbar);
    document.body.appendChild(this.hScrollbar);
    document.body.appendChild(this.scrollCorner);

    // Vertical thumb drag
    this.vThumb.addEventListener('mousedown', (e) => {
      e.preventDefault();
      this.vThumbDragging = true;
      this.vThumbStartY = e.clientY;
      this.vThumbStartOffset = this.grid?.get_scroll_metrics()
        ? JSON.parse(this.grid.get_scroll_metrics()).cell_y_offset : 0;
    });

    // Horizontal thumb drag
    this.hThumb.addEventListener('mousedown', (e) => {
      e.preventDefault();
      this.hThumbDragging = true;
      this.hThumbStartX = e.clientX;
      this.hThumbStartTranslate = this.grid?.get_scroll_metrics()
        ? JSON.parse(this.grid.get_scroll_metrics()).translate_x : 0;
    });

    window.addEventListener('mousemove', (e) => {
      if (!this.grid) return;
      if (this.vThumbDragging) {
        const metrics = JSON.parse(this.grid.get_scroll_metrics());
        const trackH = this.vScrollbar!.clientHeight - this.scrollbarWidth;
        const thumbH = Math.max(MIN_SCROLLBAR_THUMB_SIZE, trackH * Math.min(metrics.visible_rows / metrics.total_rows, 1));
        const ratio = (e.clientY - this.vThumbStartY) / (trackH - thumbH);
        const newOffset = Math.round(Math.max(0, Math.min(
          metrics.total_rows - metrics.visible_rows,
          this.vThumbStartOffset + ratio * metrics.total_rows
        )));
        this.grid.set_scroll(newOffset, 0);
        this.grid.render(); this.updateScrollbars();
      }
      if (this.hThumbDragging) {
        const metrics = JSON.parse(this.grid.get_scroll_metrics());
        const visW = metrics.canvas_width - (this.vScrollbar ? this.scrollbarWidth : 0);
        const trackW = this.hScrollbar!.clientWidth - this.scrollbarWidth;
        const thumbW = Math.max(MIN_SCROLLBAR_THUMB_SIZE, trackW * Math.min(visW / metrics.total_col_width, 1));
        const ratio = (e.clientX - this.hThumbStartX) / (trackW - thumbW);
        const newTx = Math.max(
          -(metrics.total_col_width - visW),
          Math.min(0, this.hThumbStartTranslate + ratio * (metrics.total_col_width - visW))
        );
        this.grid.set_translate(newTx, 0);
        this.grid.render(); this.updateScrollbars();
      }
    });

    window.addEventListener('mouseup', () => {
      this.vThumbDragging = false;
      this.hThumbDragging = false;
    });

    // Click on track (not thumb) = page scroll
    this.vScrollbar.addEventListener('click', (e) => {
      if (!this.grid || e.target === this.vThumb) return;
      const metrics = JSON.parse(this.grid.get_scroll_metrics());
      const thumbRect = this.vThumb!.getBoundingClientRect();
      const pageRows = Math.floor(metrics.visible_rows);
      const newOffset = e.clientY < thumbRect.top
        ? Math.max(0, metrics.cell_y_offset - pageRows)
        : Math.min(metrics.total_rows - pageRows, metrics.cell_y_offset + pageRows);
      this.grid.set_scroll(newOffset, 0);
      this.grid.render(); this.updateScrollbars();
    });

    this.hScrollbar.addEventListener('click', (e) => {
      if (!this.grid || e.target === this.hThumb) return;
      const metrics = JSON.parse(this.grid.get_scroll_metrics());
      const visW = metrics.canvas_width - (this.vScrollbar ? this.scrollbarWidth : 0);
      const thumbRect = this.hThumb!.getBoundingClientRect();
      const newTx = e.clientX < thumbRect.left
        ? Math.min(0, metrics.translate_x + visW)
        : Math.max(-(metrics.total_col_width - visW), metrics.translate_x - visW);
      this.grid.set_translate(newTx, 0);
      this.grid.render(); this.updateScrollbars();
    });
  }

  private positionScrollbars(): void {
    if (!this.vScrollbar || !this.hScrollbar || !this.scrollCorner) return;
    const rect = this.canvas.getBoundingClientRect();
    const sbw = this.scrollbarWidth;
    const totalW = rect.width + sbw;  // canvas + vscrollbar
    const totalH = rect.height + sbw; // canvas + hscrollbar

    this.vScrollbar.style.left = `${rect.right}px`;
    this.vScrollbar.style.top = `${rect.top}px`;
    this.vScrollbar.style.height = `${rect.height}px`;

    this.hScrollbar.style.left = `${rect.left}px`;
    this.hScrollbar.style.top = `${rect.bottom}px`;
    this.hScrollbar.style.width = `${rect.width}px`;

    this.scrollCorner.style.left = `${rect.right}px`;
    this.scrollCorner.style.top = `${rect.bottom}px`;
  }

  private updateScrollbars(): void {
    if (!this.grid || !this.vScrollbar || !this.vThumb || !this.hScrollbar || !this.hThumb) return;
    this.positionScrollbars();

    const metricsStr = this.grid.get_scroll_metrics();
    if (!metricsStr) return;
    const m = JSON.parse(metricsStr);

    const sbw = this.scrollbarWidth;

    // Vertical
    const vTrackH = this.vScrollbar.clientHeight;
    const vRatio = m.total_rows > 0 ? Math.min(m.visible_rows / m.total_rows, 1) : 1;
    const vThumbH = Math.max(MIN_SCROLLBAR_THUMB_SIZE, vTrackH * vRatio);
    const vScrollRange = m.total_rows - m.visible_rows;
    const vThumbTop = vScrollRange > 0
      ? (m.cell_y_offset / vScrollRange) * (vTrackH - vThumbH)
      : 0;
    this.vThumb.style.height = `${vThumbH}px`;
    this.vThumb.style.top = `${vThumbTop}px`;

    // Horizontal — visible data width excludes the vertical scrollbar overlay
    const visibleW = m.canvas_width - (this.vScrollbar ? this.scrollbarWidth : 0);
    const hTrackW = this.hScrollbar.clientWidth;
    const hRatio = m.total_col_width > 0 ? Math.min(visibleW / m.total_col_width, 1) : 1;
    const hThumbW = Math.max(MIN_SCROLLBAR_THUMB_SIZE, hTrackW * hRatio);
    const hScrollRange = m.total_col_width - visibleW;
    const hThumbLeft = hScrollRange > 0
      ? (-m.translate_x / hScrollRange) * (hTrackW - hThumbW)
      : 0;
    this.hThumb.style.width = `${hThumbW}px`;
    this.hThumb.style.left = `${Math.max(0, hThumbLeft)}px`;
  }

  // --- Format picker helpers ---

  private showFormatSubmenu(parentItem: HTMLElement, colIdx: number): void {
    if (!this.grid) return;

    // Remove any existing format submenu
    document.querySelector('.grid-format-sub')?.remove();

    const formatOpts = JSON.parse(this.grid.get_format_options(colIdx));
    if (!formatOpts.compatible_formats || formatOpts.compatible_formats.length === 0) return;

    const sub = document.createElement('div');
    sub.className = 'grid-header-menu grid-format-sub';
    sub.style.position = 'fixed';
    sub.style.minWidth = '210px';
    document.body.appendChild(sub);

    // Tracks which format type is currently selected (mutable — updated when user clicks a row)
    // Use full spec JSON as the active key to disambiguate formats that share a type
    // (e.g. multiple dateTime formats with different format strings).
    const specKey = (spec: any): string => spec === null ? 'null' : JSON.stringify(spec);
    let activeSpecKey: string = specKey(formatOpts.current_format ?? null);

    // Shared decimal counter — seeded from current format's decimals, or the first
    // has_decimals entry's default, or 2.
    let sharedDecimals: number = formatOpts.current_format?.decimals
      ?? formatOpts.compatible_formats.find((f: any) => f.has_decimals)?.spec?.decimals
      ?? 2;

    // Keep refs to all decimal-count display spans so we can update them together
    const decimalCountSpans: HTMLSpanElement[] = [];

    // Keep refs to all items so we can toggle the active checkmark
    const itemEls: Array<{ el: HTMLElement; spec: any }> = [];

    const setActive = (spec: any) => {
      activeSpecKey = specKey(spec);
      for (const { el, spec: itemSpec } of itemEls) {
        // For formats with decimals, compare by type only (decimals vary via spinner)
        const itemKey = (itemSpec && 'decimals' in itemSpec)
          ? (itemSpec.type ?? null)
          : specKey(itemSpec);
        const curKey = (spec && 'decimals' in spec)
          ? (spec.type ?? null)
          : activeSpecKey;
        el.classList.toggle('checked', itemKey === curKey);
      }
    };

    const applyFormat = (spec: any) => {
      if (!this.grid) return;
      const applied = spec === null ? null
        : ('decimals' in spec) ? { ...spec, decimals: sharedDecimals }
        : spec;
      this.grid.set_column_format(colIdx, applied === null ? 'null' : JSON.stringify(applied));
      this.grid.render();
      setActive(applied);
    };

    const updateDecimalDisplays = () => {
      for (const span of decimalCountSpans) {
        span.textContent = String(sharedDecimals);
      }
    };

    for (const fmt of formatOpts.compatible_formats) {
      const item = document.createElement('div');
      item.className = 'grid-header-menu-item';
      item.style.cssText = 'display:flex; align-items:center; gap:6px; padding:7px 14px; cursor:pointer; user-select:none;';

      // For decimal formats match by type; for others (date/dateTime) match full spec
      const isActive = fmt.spec && 'decimals' in fmt.spec
        ? (fmt.spec.type ?? null) === (formatOpts.current_format?.type ?? null)
        : specKey(fmt.spec ?? null) === activeSpecKey;
      if (isActive) item.classList.add('checked');

      itemEls.push({ el: item, spec: fmt.spec });

      // Label
      const labelEl = document.createElement('span');
      labelEl.textContent = fmt.label;
      labelEl.style.flex = '1';
      item.appendChild(labelEl);

      if (fmt.has_decimals) {
        // Decimal count display
        const countSpan = document.createElement('span');
        countSpan.textContent = String(sharedDecimals);
        countSpan.style.cssText = 'min-width:1.4em; text-align:right; color:#737383; font-size:12px;';
        decimalCountSpans.push(countSpan);
        item.appendChild(countSpan);

        // − button
        const btnStyle = 'border:1px solid #ccc; background:#fff; cursor:pointer; padding:0 5px; border-radius:3px; font-size:12px; line-height:18px; flex-shrink:0;';
        const btnMinus = document.createElement('button');
        btnMinus.textContent = '−';
        btnMinus.style.cssText = btnStyle;
        btnMinus.addEventListener('mousedown', (e) => {
          e.stopPropagation();
          e.preventDefault();
          sharedDecimals = Math.max(0, sharedDecimals - 1);
          updateDecimalDisplays();
          // Switch to this row's format type when pressing ± on it
          applyFormat(fmt.spec);
        });

        // + button
        const btnPlus = document.createElement('button');
        btnPlus.textContent = '+';
        btnPlus.style.cssText = btnStyle;
        btnPlus.addEventListener('mousedown', (e) => {
          e.stopPropagation();
          e.preventDefault();
          sharedDecimals = Math.min(MAX_DECIMAL_PLACES, sharedDecimals + 1);
          updateDecimalDisplays();
          // Switch to this row's format type when pressing ± on it
          applyFormat(fmt.spec);
        });

        item.appendChild(btnMinus);
        item.appendChild(btnPlus);
      }

      // Clicking the row (not a button) selects this format
      item.addEventListener('mousedown', (e) => {
        if ((e.target as HTMLElement).tagName === 'BUTTON') return;
        e.stopPropagation();
        e.preventDefault();
        applyFormat(fmt.spec);
      });

      item.addEventListener('mouseover', () => { item.style.background = '#f0f0f5'; });
      item.addEventListener('mouseout',  () => { item.style.background = ''; });

      sub.appendChild(item);
    }

    // Position to the right of the "Format column ›" item
    const pRect = parentItem.getBoundingClientRect();
    const left = pRect.right + SUBMENU_X_GAP;
    const top = pRect.top;
    const vw = window.innerWidth;
    const subW = sub.offsetWidth || 220;
    sub.style.left = `${Math.min(left, vw - subW - MENU_VIEWPORT_MARGIN)}px`;
    sub.style.top = `${top}px`;

    // Close when clicking outside both the submenu and the parent header menu
    const outsideHandler = (e: MouseEvent) => {
      if (!sub.contains(e.target as Node) && !this.headerMenuEl?.contains(e.target as Node)) {
        sub.remove();
        document.removeEventListener('mousedown', outsideHandler);
      }
    };
    // Delay to avoid the current mousedown immediately closing it
    setTimeout(() => document.addEventListener('mousedown', outsideHandler), 0);
  }

  private showSpanMenu(clientX: number, clientY: number, ctx: any): void {
    if (!this.grid) return;
    this.hideHeaderMenu();
    this.ensureMenuStyle();

    const { arrow_name, display_name, current_aggs, available_to_add } = ctx;
    if (!available_to_add || available_to_add.length === 0) return;

    const menu = document.createElement('div');
    menu.className = 'grid-header-menu';

    const addSection = (label: string) => {
      const el = document.createElement('div');
      el.className = 'grid-header-menu-section';
      el.textContent = label;
      menu.appendChild(el);
    };

    addSection(`Add aggregation to "${display_name}"`);

    for (const fn of available_to_add as string[]) {
      const item = document.createElement('div');
      item.className = 'grid-header-menu-item';
      item.textContent = fn.charAt(0).toUpperCase() + fn.slice(1);
      item.addEventListener('mousedown', async (e) => {
        e.stopPropagation();
        this.hideHeaderMenu();
        const newFns = [...(current_aggs as string[]), fn];
        await this.grid!.set_column_aggregations(arrow_name, JSON.stringify(newFns));
        this.grid!.render(); this.updateScrollbars();
      });
      menu.appendChild(item);
    }

    document.body.appendChild(menu);
    this.headerMenuEl = menu;
    const vw = window.innerWidth, vh = window.innerHeight;
    const mw = menu.offsetWidth || MENU_FALLBACK_WIDTH, mh = menu.offsetHeight || 200;
    menu.style.left = `${Math.min(clientX, vw - mw - MENU_VIEWPORT_MARGIN)}px`;
    menu.style.top = `${Math.min(clientY + MENU_Y_OFFSET, vh - mh - MENU_VIEWPORT_MARGIN)}px`;

    this.headerMenuCloseHandler = (e: MouseEvent) => {
      if (!menu.contains(e.target as Node)) this.hideHeaderMenu();
    };
    setTimeout(() => {
      window.addEventListener('mousedown', this.headerMenuCloseHandler!);
    }, 0);
  }

  private ensureColoris(): void {
    if (this.colorisInitialized) return;
    Coloris.init();
    Coloris({ el: '.grid-coloris-input', theme: 'pill', formatToggle: false });
    this.colorisInitialized = true;
  }

  private hideCFPanel(): void {
    if (this.cfPanelEl) { this.cfPanelEl.remove(); this.cfPanelEl = null; }
  }

  /** Show the conditional format panel as a submenu to the right of parentItem. */
  private showConditionalFormatPanel(parentItem: HTMLElement, colIdx: number, isNumeric: boolean): void {
    if (!this.grid) return;
    document.querySelector('.grid-format-sub')?.remove();
    this.hideCFPanel();
    this.ensureColoris();

    // ── Load existing rules to pre-populate the panel ──────────────────────────
    const existingRulesJson = this.grid.get_conditional_formats(colIdx);
    const existingRules: any[] = JSON.parse(existingRulesJson);
    const existingGradient = existingRules.find(r => r.type === 'gradient');
    const existingValueColor = existingRules.find(r => r.type === 'valueColor');

    // Determine initial mode from existing rules; fall back to default
    let mode: 'gradient' | 'value' =
      existingGradient ? 'gradient' :
      existingValueColor ? 'value' :
      isNumeric ? 'gradient' : 'value';

    const panel = document.createElement('div');
    panel.className = 'grid-cf-panel';
    panel.style.cssText = `position:fixed; background:#fff; border:1px solid #d0d0d8; border-radius:8px;
      box-shadow:0 4px 20px rgba(0,0,0,0.15); width:310px; z-index:10002; padding:14px;
      font-family:Inter,-apple-system,sans-serif; font-size:13px; color:#313139;`;
    this.cfPanelEl = panel;
    document.body.appendChild(panel);

    // Mode selector
    const modeRow = document.createElement('div');
    modeRow.style.cssText = 'display:flex; gap:10px; margin-bottom:12px;';
    const mkRadio = (label: string, value: 'gradient' | 'value') => {
      const lbl = document.createElement('label');
      lbl.style.cssText = 'display:flex; align-items:center; gap:4px; cursor:pointer; font-size:13px;';
      const inp = document.createElement('input');
      inp.type = 'radio'; inp.name = `cf-mode-${colIdx}`; inp.value = value; inp.checked = mode === value;
      inp.addEventListener('change', () => { mode = value; contentArea.innerHTML = ''; renderContent(); });
      lbl.appendChild(inp); lbl.appendChild(document.createTextNode(label));
      return lbl;
    };
    if (isNumeric) modeRow.appendChild(mkRadio('Gradient', 'gradient'));
    modeRow.appendChild(mkRadio('Value-based', 'value'));
    panel.appendChild(modeRow);

    const contentArea = document.createElement('div');
    panel.appendChild(contentArea);

    let getPendingRules: () => any[] = () => [];

    const mkBtn = (label: string, primary: boolean, onClick: () => void) => {
      const btn = document.createElement('button');
      btn.textContent = label;
      btn.style.cssText = `padding:5px 14px; border-radius:4px; cursor:pointer; font-size:13px; border:1px solid #d0d0d8;
        background:${primary ? '#4F5DFF' : '#fff'}; color:${primary ? '#fff' : '#313139'};`;
      btn.addEventListener('mousedown', (e) => { e.stopPropagation(); e.preventDefault(); onClick(); });
      return btn;
    };

    const btnRow = document.createElement('div');
    btnRow.style.cssText = 'display:flex; gap:8px; margin-top:12px; justify-content:flex-end;';
    btnRow.appendChild(mkBtn('Clear', false, () => {
      if (!this.grid) return;
      this.grid.set_conditional_formats(colIdx, '[]');
      this.grid.render(); this.updateScrollbars();
      this.hideCFPanel(); this.hideHeaderMenu();
    }));
    btnRow.appendChild(mkBtn('Apply', true, () => {
      if (!this.grid) return;
      this.grid.set_conditional_formats(colIdx, JSON.stringify(getPendingRules()));
      this.grid.render(); this.updateScrollbars();
      this.hideCFPanel(); this.hideHeaderMenu();
    }));
    panel.appendChild(btnRow);

    // ── Color swatch helper — swatch button triggers Coloris, no visible <input> ──
    const mkColorSwatch = (initialColor: string, onChange: (c: string) => void): HTMLElement => {
      const wrap = document.createElement('div');
      wrap.style.cssText = 'position:relative; display:inline-block;';

      // Hidden input that Coloris attaches to
      const hidden = document.createElement('input');
      hidden.type = 'text';
      hidden.value = initialColor;
      hidden.className = 'grid-coloris-input';
      hidden.style.cssText = 'position:absolute; opacity:0; pointer-events:none; width:1px; height:1px;';
      hidden.addEventListener('change', () => onChange(hidden.value));

      // Visible swatch button
      const swatch = document.createElement('button');
      swatch.type = 'button';
      swatch.style.cssText = `width:28px; height:28px; border-radius:4px; border:1px solid #ccc;
        cursor:pointer; background:${initialColor}; padding:0; flex-shrink:0;`;
      swatch.addEventListener('mousedown', (e) => {
        e.stopPropagation();
        e.preventDefault();
        hidden.click(); // open Coloris
      });

      // Keep swatch colour in sync when Coloris updates the hidden input
      const observer = new MutationObserver(() => {
        swatch.style.background = hidden.value;
      });
      hidden.addEventListener('change', () => { swatch.style.background = hidden.value; });
      // Coloris fires 'input' events too
      hidden.addEventListener('input', () => { onChange(hidden.value); swatch.style.background = hidden.value; });

      wrap.appendChild(hidden);
      wrap.appendChild(swatch);
      setTimeout(() => Coloris({ el: '.grid-coloris-input' }), 0);
      return wrap;
    };

    // ── Gradient mode ─────────────────────────────────────────────────────────
    const renderGradient = () => {
      const statsJson = this.grid!.get_column_stats(colIdx);
      const stats = statsJson !== 'null' ? JSON.parse(statsJson) : null;
      const fmt = (v: number) => String(parseFloat(v.toFixed(4)));

      // Seed from existing rules if available, otherwise use actual min/max
      let lowColor  = existingGradient?.lowColor  ?? '#63be7b';
      let highColor = existingGradient?.highColor ?? '#f8696b';
      let minValStr = existingGradient?.minValue !== undefined
        ? fmt(existingGradient.minValue)
        : (stats ? fmt(stats.min) : '');
      let maxValStr = existingGradient?.maxValue !== undefined
        ? fmt(existingGradient.maxValue)
        : (stats ? fmt(stats.max) : '');

      const labelStyle = 'font-size:12px; color:#737383; margin-bottom:3px; display:block;';
      const inputStyle = 'border:1px solid #ccc; border-radius:4px; padding:4px 6px; font-size:12px;';

      const addRow = (valueLabel: string, colorLabel: string, colorInit: string, valueInit: string,
                      onColor: (c: string) => void, onValue: (v: string) => void) => {
        const row = document.createElement('div');
        // value field on left, color swatch on right, vertically centered
        row.style.cssText = 'display:flex; align-items:center; gap:10px; margin-bottom:10px;';

        const leftWrap = document.createElement('div');
        leftWrap.style.cssText = 'flex:1;';
        const leftLbl = document.createElement('span');
        leftLbl.style.cssText = labelStyle;
        leftLbl.textContent = valueLabel;
        const valueInp = document.createElement('input');
        valueInp.type = 'text'; valueInp.value = valueInit; valueInp.placeholder = 'Auto';
        valueInp.style.cssText = inputStyle + 'width:100%; box-sizing:border-box;';
        valueInp.addEventListener('input', () => onValue(valueInp.value));
        leftWrap.appendChild(leftLbl);
        leftWrap.appendChild(valueInp);

        const rightWrap = document.createElement('div');
        rightWrap.style.cssText = 'display:flex; flex-direction:column; align-items:center; gap:2px; flex-shrink:0;';
        const rightLbl = document.createElement('span');
        rightLbl.style.cssText = labelStyle + 'text-align:center;';
        rightLbl.textContent = colorLabel;
        const swatch = mkColorSwatch(colorInit, onColor);
        rightWrap.appendChild(rightLbl);
        rightWrap.appendChild(swatch);

        row.appendChild(leftWrap);
        row.appendChild(rightWrap);
        contentArea.appendChild(row);
      };

      addRow('Min value', 'Color', lowColor, minValStr, c => { lowColor = c; }, v => { minValStr = v; });
      addRow('Max value', 'Color', highColor, maxValStr, c => { highColor = c; }, v => { maxValStr = v; });

      getPendingRules = () => {
        const minV = minValStr.trim() !== '' ? parseFloat(minValStr) : undefined;
        const maxV = maxValStr.trim() !== '' ? parseFloat(maxValStr) : undefined;
        return [{ type: 'gradient', lowColor, highColor,
          ...(minV !== undefined && !isNaN(minV) ? { minValue: minV } : {}),
          ...(maxV !== undefined && !isNaN(maxV) ? { maxValue: maxV } : {}),
        }];
      };
    };

    // ── Value-based mode ──────────────────────────────────────────────────────
    const renderValueBased = () => {
      const uniqueData = JSON.parse(this.grid!.get_column_unique_values(colIdx));
      const allValues: string[] = uniqueData.values;
      // Seed from existing rules
      let valueRules: Array<{ value: string; bgColor: string }> =
        existingValueColor?.rules?.map((r: any) => ({ value: r.value, bgColor: r.bgColor })) ?? [];

      const addRuleBtn = document.createElement('button');
      addRuleBtn.textContent = '+ Add rule';
      addRuleBtn.style.cssText = 'border:1px solid #ccc; background:#fff; border-radius:4px; padding:4px 10px; cursor:pointer; font-size:12px; margin-bottom:8px;';
      contentArea.appendChild(addRuleBtn);

      const rulesList = document.createElement('div');
      rulesList.style.maxHeight = '180px';
      rulesList.style.overflowY = 'auto';
      contentArea.appendChild(rulesList);

      const refreshRules = () => {
        rulesList.innerHTML = '';
        for (let i = 0; i < valueRules.length; i++) {
          const rule = valueRules[i];
          const row = document.createElement('div');
          row.style.cssText = 'display:flex; align-items:center; gap:6px; margin-bottom:6px;';

          // Value combo input + dropdown
          const valueWrap = document.createElement('div');
          valueWrap.style.cssText = 'position:relative; flex:1;';
          const valueInp = document.createElement('input');
          valueInp.type = 'text'; valueInp.value = rule.value; valueInp.placeholder = 'Value...';
          valueInp.style.cssText = 'width:100%; border:1px solid #ccc; border-radius:4px; padding:4px 6px; font-size:12px; box-sizing:border-box;';
          const dropdown = document.createElement('ul');
          dropdown.style.cssText = `position:absolute; top:100%; left:0; right:0; background:#fff;
            border:1px solid #ccc; border-radius:4px; max-height:130px; overflow-y:auto;
            z-index:10010; list-style:none; margin:2px 0; padding:0; display:none;`;
          const showDropdown = (filter: string) => {
            const matches = allValues.filter(v => v.toLowerCase().includes(filter.toLowerCase())).slice(0, 50);
            dropdown.innerHTML = '';
            matches.forEach(val => {
              const li = document.createElement('li');
              li.textContent = val;
              li.style.cssText = 'padding:5px 8px; cursor:pointer; font-size:12px;';
              li.addEventListener('mouseover', () => li.style.background = '#f0f0f5');
              li.addEventListener('mouseout',  () => li.style.background = '');
              li.addEventListener('mousedown', (e) => {
                e.preventDefault(); valueInp.value = val; rule.value = val; dropdown.style.display = 'none';
              });
              dropdown.appendChild(li);
            });
            dropdown.style.display = matches.length ? 'block' : 'none';
          };
          valueInp.addEventListener('focus', () => showDropdown(valueInp.value));
          valueInp.addEventListener('input', () => { showDropdown(valueInp.value); rule.value = valueInp.value; });
          valueInp.addEventListener('blur', () => setTimeout(() => { dropdown.style.display = 'none'; }, 150));
          valueWrap.appendChild(valueInp);
          valueWrap.appendChild(dropdown);

          // Color swatch (no visible input)
          const swatch = mkColorSwatch(rule.bgColor, c => { rule.bgColor = c; });

          const idx2 = i;
          const removeBtn = document.createElement('button');
          removeBtn.textContent = '×';
          removeBtn.style.cssText = 'border:none; background:none; cursor:pointer; font-size:16px; color:#737383; padding:0 4px;';
          removeBtn.addEventListener('mousedown', (e) => { e.preventDefault(); valueRules.splice(idx2, 1); refreshRules(); });

          row.appendChild(valueWrap);
          row.appendChild(swatch);
          row.appendChild(removeBtn);
          rulesList.appendChild(row);
        }
      };

      getPendingRules = () =>
        valueRules.length ? [{ type: 'valueColor', rules: valueRules.map(r => ({ value: r.value, bgColor: r.bgColor })) }] : [];

      addRuleBtn.addEventListener('mousedown', (e) => {
        e.preventDefault(); valueRules.push({ value: '', bgColor: '#ffff00' }); refreshRules();
      });
      refreshRules();
    };

    const renderContent = () => { if (mode === 'gradient') renderGradient(); else renderValueBased(); };
    renderContent();

    // Position panel to the right of the parent menu item
    const pRect = parentItem.getBoundingClientRect();
    const vw = window.innerWidth, vh = window.innerHeight;
    const left = Math.min(pRect.right + SUBMENU_X_GAP, vw - 310 - MENU_VIEWPORT_MARGIN);
    const top  = Math.min(pRect.top, vh - 400);
    panel.style.left = `${left}px`;
    panel.style.top  = `${top}px`;

    // Close on outside click. Track the handler so hideCFPanel can't leave it dangling.
    const outsideHandler = (e: MouseEvent) => {
      if (!panel.contains(e.target as Node) && !this.headerMenuEl?.contains(e.target as Node)) {
        this.hideCFPanel();
      }
    };
    // Small delay so the mouseenter that opened this panel doesn't immediately trigger the handler
    const timerId = setTimeout(() => document.addEventListener('mousedown', outsideHandler), 100);

    // Patch hideCFPanel for this instance to also clean up the listener
    const origHide = this.hideCFPanel.bind(this);
    this.hideCFPanel = () => {
      clearTimeout(timerId);
      document.removeEventListener('mousedown', outsideHandler);
      this.hideCFPanel = origHide; // restore
      origHide();
    };
  }

  private showColDragChip(clientX: number, clientY: number): void {
    if (!this.grid) return;
    if (!this.colDragChip) {
      const chip = document.createElement('div');
      chip.style.position = 'fixed';
      chip.style.background = '#fff';
      chip.style.border = '1px solid #ccc';
      chip.style.borderRadius = '4px';
      chip.style.padding = '4px 10px';
      chip.style.fontSize = '13px';
      chip.style.fontFamily = 'Inter, sans-serif';
      chip.style.fontWeight = '600';
      chip.style.boxShadow = '0 2px 8px rgba(0,0,0,0.15)';
      chip.style.pointerEvents = 'none';
      chip.style.zIndex = COL_DRAG_CHIP_Z_INDEX;
      chip.style.whiteSpace = 'nowrap';
      chip.textContent = this.grid.get_drag_col_name();
      document.body.appendChild(chip);
      this.colDragChip = chip;
    }
    this.colDragChip.style.left = `${clientX + COL_DRAG_CHIP_X_OFFSET}px`;
    this.colDragChip.style.top = `${clientY - COL_DRAG_CHIP_Y_OFFSET}px`;
  }

  private hideColDragChip(): void {
    if (this.colDragChip) {
      this.colDragChip.remove();
      this.colDragChip = null;
    }
  }

  private ensureMenuStyle(): void {
    if (this.menuStyle) return;
    const style = document.createElement('style');
    style.textContent = `
      .grid-header-menu { position:fixed; background:#fff; border:1px solid #d0d0d8; border-radius:6px;
        box-shadow:0 4px 16px rgba(0,0,0,0.12); min-width:180px; z-index:10001; overflow:hidden;
        font-family:Inter,-apple-system,sans-serif; font-size:13px; color:#313139; }
      .grid-header-menu-item { padding:8px 14px; cursor:pointer; display:flex; align-items:center; gap:8px; }
      .grid-header-menu-item:hover { background:#f0f0f5; }
      .grid-header-menu-item.checked::before { content:'✓'; width:14px; }
      .grid-header-menu-item:not(.checked)::before { content:''; width:14px; display:inline-block; }
      .grid-header-menu-section { padding:6px 14px 2px; font-size:11px; font-weight:600;
        color:#737383; text-transform:uppercase; letter-spacing:0.05em; }
      .grid-header-menu-separator { height:1px; background:#e8e8ed; margin:4px 0; }
      .grid-header-menu-item.danger { color:#c0392b; }
    `;
    document.head.appendChild(style);
    this.menuStyle = style;
  }

  private showHeaderMenu(clientX: number, clientY: number, ctx: any): void {
    if (!this.grid) return;
    this.hideHeaderMenu();
    this.ensureMenuStyle();

    const menu = document.createElement('div');
    menu.className = 'grid-header-menu';

    const addSection = (label: string) => {
      const el = document.createElement('div');
      el.className = 'grid-header-menu-section';
      el.textContent = label;
      menu.appendChild(el);
    };

    const addItem = (label: string, checked: boolean | null, action: () => void, danger = false) => {
      const el = document.createElement('div');
      el.className = 'grid-header-menu-item' + (checked ? ' checked' : '') + (danger ? ' danger' : '');
      if (checked === null) el.style.paddingLeft = '28px';
      el.textContent = label;
      el.addEventListener('mousedown', async (e) => {
        e.stopPropagation();
        this.hideHeaderMenu();
        await action();
      });
      menu.appendChild(el);
    };

    const addSeparator = () => {
      const el = document.createElement('div');
      el.className = 'grid-header-menu-separator';
      menu.appendChild(el);
    };

    const { arrow_name, display_name, display_index: ctx_display_index, is_group_key, is_grouped_mode, current_aggs, compatible_aggs, this_agg_fn, sibling_agg_count, can_group_by, is_mandatory_group_key } = ctx;

    const truncOpts = this.grid.get_date_truncation_options(ctx_display_index);
    const truncData = truncOpts !== 'null' ? JSON.parse(truncOpts) : null;

    const attachDateTruncSubmenu = (parentItem: HTMLElement, includeRemove: boolean) => {
      parentItem.addEventListener('mouseenter', () => {
        document.querySelector('.grid-date-trunc-sub')?.remove();
        const sub = document.createElement('div');
        sub.className = 'grid-header-menu grid-date-trunc-sub';
        sub.style.cssText = 'position:fixed; min-width:160px; z-index:10003;';
        document.body.appendChild(sub);

        const active: string[] = truncData.active ?? [];
        for (const t of truncData.available as string[]) {
          const li = document.createElement('div');
          li.className = 'grid-header-menu-item' + (active.includes(t) ? ' checked' : '');
          li.textContent = t.charAt(0).toUpperCase() + t.slice(1);
          li.addEventListener('mousedown', async (e) => {
            e.stopPropagation();
            this.hideHeaderMenu();
            sub.remove();
            await this.grid!.toggle_group_key_truncated(arrow_name, t);
            this.grid!.render(); this.updateScrollbars();
          });
          sub.appendChild(li);
        }

        const rawActive = (truncData.active_raw ?? false);
        const rawLi = document.createElement('div');
        rawLi.className = 'grid-header-menu-item' + (rawActive ? ' checked' : '');
        rawLi.textContent = 'No truncation';
        rawLi.addEventListener('mousedown', async (e) => {
          e.stopPropagation();
          this.hideHeaderMenu();
          sub.remove();
          await this.grid!.toggle_group_key(arrow_name);
          this.grid!.render(); this.updateScrollbars();
        });
        sub.appendChild(rawLi);

        if (includeRemove) {
          const sep = document.createElement('div');
          sep.className = 'grid-header-menu-separator';
          sub.appendChild(sep);
          const removeLi = document.createElement('div');
          removeLi.className = 'grid-header-menu-item danger';
          removeLi.textContent = `Remove "${display_name}" from grouping`;
          removeLi.addEventListener('mousedown', async (e) => {
            e.stopPropagation();
            this.hideHeaderMenu();
            sub.remove();
            await this.grid!.clear_group_key(arrow_name);
            this.grid!.render(); this.updateScrollbars();
          });
          sub.appendChild(removeLi);
        }

        const pRect = parentItem.getBoundingClientRect();
        sub.style.left = `${pRect.right + SUBMENU_X_GAP}px`;
        sub.style.top = `${pRect.top}px`;
      });
      parentItem.addEventListener('mouseleave', () => {
        setTimeout(() => {
          const sub = document.querySelector('.grid-date-trunc-sub') as HTMLElement;
          if (sub && !sub.matches(':hover')) sub.remove();
        }, 200);
      });
    };

    const makeDateTruncItem = (label: string, includeRemove: boolean) => {
      const gbItem = document.createElement('div');
      gbItem.className = 'grid-header-menu-item';
      gbItem.style.cssText = 'display:flex; justify-content:space-between; align-items:center;';
      gbItem.appendChild(Object.assign(document.createElement('span'), { textContent: label }));
      gbItem.appendChild(Object.assign(document.createElement('span'), { textContent: '›', style: 'font-weight:300;font-size:16px;' }));
      attachDateTruncSubmenu(gbItem, includeRemove);
      return gbItem;
    };

    if (can_group_by) {
      if (truncData && truncData.available.length > 0) {
        menu.appendChild(makeDateTruncItem(`Group by "${display_name}"`, false));
      } else {
        addItem(`Group by "${display_name}"`, null, async () => {
          await this.grid!.toggle_group_key(arrow_name);
          this.grid!.render(); this.updateScrollbars();
        });
      }
    }

    if (is_grouped_mode && is_group_key && !is_mandatory_group_key) {
      if (truncData && truncData.available.length > 0) {
        menu.appendChild(makeDateTruncItem(`Group by "${display_name}"`, true));
      } else {
        addItem(`Remove "${display_name}" from grouping`, null, async () => {
          await this.grid!.toggle_group_key(arrow_name);
          this.grid!.render(); this.updateScrollbars();
        });
      }
    }

    if (is_grouped_mode && !is_group_key && this_agg_fn) {
      // Value agg leaf: radio-style selection — clicking replaces this leaf's fn
      if (menu.children.length > 0) addSeparator();
      addSection('Aggregation for this column');
      for (const fn of compatible_aggs as string[]) {
        const isThis = fn === this_agg_fn;
        addItem(fn.charAt(0).toUpperCase() + fn.slice(1), isThis, async () => {
          if (isThis) return; // already selected
          await this.grid!.replace_column_aggregation(arrow_name, this_agg_fn, fn);
          this.grid!.render(); this.updateScrollbars();
        });
      }
      // Remove option — only if 2+ agg fns on the parent column
      if ((sibling_agg_count as number) >= MIN_SIBLING_AGGS_TO_REMOVE) {
        addSeparator();
        addItem('Remove this aggregation', null, async () => {
          const newFns = (current_aggs as string[]).filter((f: string) => f !== this_agg_fn);
          await this.grid!.set_column_aggregations(arrow_name, JSON.stringify(newFns));
          this.grid!.render(); this.updateScrollbars();
        }, true);
      }
    }

    if (is_grouped_mode) {
      addSeparator();
      addItem('Clear all grouping', null, () => {
        this.grid!.clear_grouping();
        this.grid!.render(); this.updateScrollbars();
      }, true);
    }

    // Format column submenu
    const { display_index } = ctx;
    const formatOpts = JSON.parse(this.grid.get_format_options(display_index));
    if (formatOpts.compatible_formats && formatOpts.compatible_formats.length > 0) {
      if (menu.children.length > 0) addSeparator();
      const fmtItem = document.createElement('div');
      fmtItem.className = 'grid-header-menu-item';
      fmtItem.style.display = 'flex';
      fmtItem.style.justifyContent = 'space-between';
      fmtItem.style.alignItems = 'center';
      const fmtLabel = document.createElement('span');
      fmtLabel.textContent = 'Format column';
      const arrow = document.createElement('span');
      arrow.textContent = '›';
      arrow.style.fontWeight = '300';
      arrow.style.fontSize = '16px';
      fmtItem.appendChild(fmtLabel);
      fmtItem.appendChild(arrow);
      fmtItem.addEventListener('mouseenter', () => {
        this.hideCFPanel();
        this.showFormatSubmenu(fmtItem, display_index);
      });
      menu.appendChild(fmtItem);
    }

    // Conditional format option (not shown for group keys in grouped mode)
    if (!is_group_key) {
      addSeparator();
      const cfItem = document.createElement('div');
      cfItem.className = 'grid-header-menu-item';
      cfItem.style.display = 'flex';
      cfItem.style.justifyContent = 'space-between';
      cfItem.style.alignItems = 'center';
      const cfLabel = document.createElement('span');
      cfLabel.textContent = 'Conditional format';
      const cfArrow = document.createElement('span');
      cfArrow.textContent = '›';
      cfArrow.style.fontWeight = '300';
      cfArrow.style.fontSize = '16px';
      cfItem.appendChild(cfLabel);
      cfItem.appendChild(cfArrow);
      cfItem.addEventListener('mouseenter', () => {
        // Close format submenu if open — only one submenu at a time
        document.querySelector('.grid-format-sub')?.remove();
        this.showConditionalFormatPanel(cfItem, display_index, !!formatOpts.is_numeric);
      });
      menu.appendChild(cfItem);
    }

    // Position menu
    document.body.appendChild(menu);
    this.headerMenuEl = menu;
    const vw = window.innerWidth, vh = window.innerHeight;
    const mw = menu.offsetWidth || MENU_FALLBACK_WIDTH, mh = menu.offsetHeight || 200;
    const left = Math.min(clientX, vw - mw - MENU_VIEWPORT_MARGIN);
    const top = Math.min(clientY + MENU_Y_OFFSET, vh - mh - MENU_VIEWPORT_MARGIN);
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;

    // Close on outside click
    this.headerMenuCloseHandler = (e: MouseEvent) => {
      if (!menu.contains(e.target as Node)) {
        this.hideHeaderMenu();
      }
    };
    setTimeout(() => {
      window.addEventListener('mousedown', this.headerMenuCloseHandler!);
    }, 0);
  }

  private hideHeaderMenu(): void {
    if (this.headerMenuEl) {
      this.headerMenuEl.remove();
      this.headerMenuEl = null;
    }
    if (this.headerMenuCloseHandler) {
      window.removeEventListener('mousedown', this.headerMenuCloseHandler);
      this.headerMenuCloseHandler = null;
    }
  }

  async toggleGroupKey(arrowName: string): Promise<void> {
    if (!this.grid) return;
    await this.grid.toggle_group_key(arrowName);
    this.grid.render();
  }

  async clearGroupKey(arrowName: string): Promise<void> {
    if (!this.grid) return;
    await this.grid.clear_group_key(arrowName);
    this.grid.render();
  }

  async setColumnAggregations(arrowName: string, aggFns: string[]): Promise<void> {
    if (!this.grid) return;
    await this.grid.set_column_aggregations(arrowName, JSON.stringify(aggFns));
    this.grid.render();
  }

  clearGrouping(): void {
    if (!this.grid) return;
    this.grid.clear_grouping();
    this.grid.render();
  }

  isGrouped(): boolean {
    return this.grid?.is_grouped() ?? false;
  }

  getGroupByState(): any {
    if (!this.grid) return null;
    try { return JSON.parse(this.grid.get_group_by_state()); } catch { return null; }
  }

  private startAnimLoop(): void {
    if (this.animFrameId !== null) return;
    const tick = () => {
      if (!this.grid) { this.animFrameId = null; return; }
      this.grid.render();
      if (this.grid.is_animating()) {
        this.animFrameId = requestAnimationFrame(tick);
      } else {
        this.animFrameId = null;
      }
    };
    this.animFrameId = requestAnimationFrame(tick);
  }

  async setData(data: DataSource): Promise<void> {
    this.options.data = data;
    await this.configure();
    this.grid?.render();
  }

  setColumns(columns: ColumnInput[]): void {
    if (!this.grid) return;
    this.grid.set_column_input(JSON.stringify(columns));
    this.grid.render();
  }

  setColumnOverrides(overrides: ColumnInput[]): void {
    if (!this.grid) return;
    this.grid.set_column_overrides(JSON.stringify(overrides));
    this.grid.render();
  }

  getSchema(): SchemaField[] {
    if (!this.grid) return [];
    try {
      return JSON.parse(this.grid.get_schema());
    } catch {
      return [];
    }
  }

  getRowCount(): number {
    return this.grid?.get_row_count() ?? 0;
  }

  getColumnCount(): number {
    return this.grid?.get_column_count() ?? 0;
  }

  async executeQuery(sql: string): Promise<void> {
    if (!this.grid) return;
    await this.grid.execute_query(sql);
  }

  initDataFusion(tableName: string = 'data'): void {
    if (!this.grid) return;
    this.grid.init_datafusion(tableName);
  }

  render(): void {
    this.grid?.render();
    this.updateScrollbars();
  }

  setColumnFormat(displayIndex: number, format: NumberFormat | null): void {
    if (!this.grid) return;
    this.grid.set_column_format(displayIndex, format === null ? 'null' : JSON.stringify(format));
    this.grid.render();
  }

  copyToClipboard(): void {
    if (!this.grid) return;
    const tsv = this.grid.get_selected_cells_tsv();
    if (tsv) {
      navigator.clipboard.writeText(tsv).catch(() => {});
    }
  }

  getSelection(): any {
    return this.grid?.get_selection() ?? null;
  }

  getSortState(): SortState | null {
    if (!this.grid) return null;
    try {
      return JSON.parse(this.grid.get_sort_state());
    } catch {
      return null;
    }
  }

  async setSortState(state: SortState | null): Promise<void> {
    if (!this.grid) return;
    if (state === null) {
      this.grid.clear_sort();
    } else {
      this.grid.set_sort_state(state.column, state.direction);
      await this.grid.apply_sort();
    }
    this.grid.render();
  }

  async clearSort(): Promise<void> {
    if (!this.grid) return;
    this.grid.clear_sort();
    this.grid.render();
  }

  destroy(): void {
    this.hideHeaderMenu();
    this.hideColDragChip();
    this.hideCFPanel();
    this.menuStyle?.remove();
    this.vScrollbar?.remove();
    this.hScrollbar?.remove();
    this.scrollCorner?.remove();
    this.observer?.disconnect();
    this.grid?.free();
    this.grid = null;
  }
}
