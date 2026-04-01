import wasmInit, { DataGrid as WasmDataGridInternal } from './wasm/grid_core.js';

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

export interface DataGridOptions {
  columns: GridColumn[];
  rows: number;
  getCellContent: (col: number, row: number) => GridCell;
  headerHeight?: number;
  rowHeight?: number;
  freezeColumns?: number;
  freezeTrailingRows?: number;
  theme?: Theme;
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

  constructor(canvas: HTMLCanvasElement, options: DataGridOptions) {
    this.canvas = canvas;
    this.options = options;
  }

  async init(): Promise<void> {
    await ensureWasmInit();
    this.grid = new WasmDataGridInternal(this.canvas);
    this.configure();
    this.attachEvents();
    this.render();
  }

  private configure(): void {
    if (!this.grid) return;

    const { columns, rows, getCellContent, headerHeight, rowHeight, freezeColumns, freezeTrailingRows, theme } = this.options;

    this.grid.set_columns(columns);
    this.grid.set_rows(rows);
    this.grid.set_cell_callback(getCellContent);

    if (headerHeight !== undefined) this.grid.set_header_height(headerHeight);
    if (rowHeight !== undefined) this.grid.set_row_height(rowHeight);
    if (freezeColumns !== undefined) this.grid.set_freeze_columns(freezeColumns);
    if (freezeTrailingRows !== undefined) this.grid.set_freeze_trailing_rows(freezeTrailingRows);
    if (theme) this.grid.set_theme(theme);

    this.updateSize();
  }

  private updateSize(): void {
    if (!this.grid) return;
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.grid.set_size(rect.width * dpr, rect.height * dpr);
  }

  private attachEvents(): void {
    this.observer = new ResizeObserver(() => {
      this.updateSize();
      this.render();
    });
    this.observer.observe(this.canvas);

    this.canvas.addEventListener('click', (e) => {
      if (!this.grid) return;
      const rect = this.canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = (e.clientX - rect.left) * dpr;
      const y = (e.clientY - rect.top) * dpr;
      this.grid.on_click(x, y, e.shiftKey, e.ctrlKey);
      this.render();
    });

    this.canvas.addEventListener('mousemove', (e) => {
      if (!this.grid) return;
      const rect = this.canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const x = (e.clientX - rect.left) * dpr;
      const y = (e.clientY - rect.top) * dpr;
      this.grid.on_mouse_move(x, y);
    });

    this.canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      if (!this.grid) return;
      this.grid.on_scroll(e.deltaX, e.deltaY);
      this.render();
    }, { passive: false });

    this.canvas.setAttribute('tabindex', '0');
    this.canvas.addEventListener('keydown', (e) => {
      if (!this.grid) return;
      this.grid.on_key_down(e.key, e.shiftKey, e.ctrlKey);
      this.render();
      e.preventDefault();
    });

    this.canvas.addEventListener('focus', () => {
      this.grid?.set_focused(true);
      this.render();
    });

    this.canvas.addEventListener('blur', () => {
      this.grid?.set_focused(false);
      this.render();
    });
  }

  render(): void {
    this.grid?.render();
  }

  setColumns(columns: GridColumn[]): void {
    this.options.columns = columns;
    this.grid?.set_columns(columns);
    this.render();
  }

  setRows(rows: number): void {
    this.options.rows = rows;
    this.grid?.set_rows(rows);
    this.render();
  }

  setTheme(theme: Theme): void {
    this.options.theme = theme;
    this.grid?.set_theme(theme);
    this.render();
  }

  getSelection(): any {
    return this.grid?.get_selection() ?? null;
  }

  destroy(): void {
    this.observer?.disconnect();
    this.grid?.free();
    this.grid = null;
  }
}
