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
  | { type: 'date'; format?: string };

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

export interface ColumnInput {
  name?: string;
  display?: string;
  initWidth?: number;
  isResizable?: boolean;
  headerStyle?: HeaderStyle;
  dataStyle?: DataStyle;
  children?: ColumnInput[];
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

    const { data, columns, columnOverrides, headerHeight, rowHeight, freezeColumns, freezeTrailingRows, theme, initialSortState } = this.options;

    if (columns && columnOverrides) {
      throw new Error("Cannot specify both 'columns' and 'columnOverrides'");
    }

    // Set layout params before data so configure_columns uses correct header height
    if (headerHeight !== undefined) this.grid.set_header_height(headerHeight);
    if (rowHeight !== undefined) this.grid.set_row_height(rowHeight);
    if (freezeColumns !== undefined) this.grid.set_freeze_columns(freezeColumns);
    if (freezeTrailingRows !== undefined) this.grid.set_freeze_trailing_rows(freezeTrailingRows);
    if (theme) this.grid.set_theme(theme);

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

    if (initialSortState) {
      this.grid.set_sort_state(initialSortState.column, initialSortState.direction);
      await this.grid.apply_sort();
    }

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
      this.grid?.render();
    });
    this.observer.observe(this.canvas);

    this.canvas.addEventListener('mousedown', (e) => {
      if (!this.grid) return;
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
      this.grid.render();
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
        this.grid.render();
        return;
      }

      if (this.isDragging) {
        const dx = x - this.mouseDownX;
        const dy = y - this.mouseDownY;
        if (!this.dragMoved && (Math.abs(dx) > 4 || Math.abs(dy) > 4)) {
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
    });

    window.addEventListener('mouseup', (e) => {
      if (!this.grid) return;
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
        this.grid.render();
        return;
      }

      if (this.isDragging) {
        if (this.dragMoved) {
          this.grid.on_drag_end(x, y, true);
        } else if (!this.mouseDownShift && !this.mouseDownCtrl) {
          this.grid.on_drag_end(x, y, false);
        }
        this.isDragging = false;
        this.grid.render();
        return;
      }

      if (this.isResizing) {
        this.grid.on_mouse_up(x, y);
        this.isResizing = false;
        this.canvas.style.cursor = 'default';
        this.grid.render();
      }
    });

    this.canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      if (!this.grid) return;
      this.grid.on_scroll(e.deltaX, e.deltaY);
      this.grid.render();
    }, { passive: false });

    this.canvas.setAttribute('tabindex', '0');
    this.canvas.addEventListener('keydown', (e) => {
      if (!this.grid) return;
      this.grid.on_key_down(e.key, e.shiftKey, e.ctrlKey);
      this.grid.render();
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
      chip.style.zIndex = '10000';
      chip.style.whiteSpace = 'nowrap';
      chip.textContent = this.grid.get_drag_col_name();
      document.body.appendChild(chip);
      this.colDragChip = chip;
    }
    this.colDragChip.style.left = `${clientX + 12}px`;
    this.colDragChip.style.top = `${clientY - 14}px`;
  }

  private hideColDragChip(): void {
    if (this.colDragChip) {
      this.colDragChip.remove();
      this.colDragChip = null;
    }
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
    this.observer?.disconnect();
    this.grid?.free();
    this.grid = null;
  }
}
