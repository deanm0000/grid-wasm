import { useEffect, useRef } from 'react'
import { WasmDataGrid, type GridColumn, type GridCell, type Theme } from './grid'
import './App.css'

const NUM_ROWS = 100_000
const NUM_COLS = 26

const columns: GridColumn[] = Array.from({ length: NUM_COLS }, (_, i) => ({
  title: `Column ${String.fromCharCode(65 + i)}`,
  width: 150,
}))

function getCellContent(col: number, row: number): GridCell {
  if (row < 0) return { kind: 'text', data: '' }

  if (col % 3 === 0) {
    const val = (row * 17 + col * 31) % 10000
    return {
      kind: 'number',
      data: val,
      displayData: val.toLocaleString(),
      contentAlign: 'right',
    }
  }

  const words = ['alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta', 'theta', 'kappa', 'sigma', 'omega']
  const wordIdx = (row * 7 + col * 13) % words.length
  const prefix = String.fromCharCode(65 + (row % 26))
  return {
    kind: 'text',
    data: `${prefix}-${words[wordIdx]}-${row}`,
    displayData: `${prefix}-${words[wordIdx]}-${row}`,
  }
}

const theme: Theme = {
  accent_color: '#4F5DFF',
  accent_fg: '#FFFFFF',
  accent_light: 'rgba(62, 116, 253, 0.1)',
  text_dark: '#313139',
  text_medium: '#737383',
  text_light: '#B2B2C0',
  text_bubble: '#313139',
  bg_icon_header: '#737383',
  fg_icon_header: '#FFFFFF',
  text_header: '#313139',
  text_header_selected: '#FFFFFF',
  bg_cell: '#FFFFFF',
  bg_cell_medium: '#FAFAFB',
  bg_header: '#F7F7F8',
  bg_header_has_focus: '#E9E9EB',
  bg_header_hovered: '#EFEFF1',
  bg_bubble: '#EDEDF3',
  bg_bubble_selected: '#FFFFFF',
  bg_search_result: '#fff9e3',
  border_color: 'rgba(115, 116, 131, 0.16)',
  drilldown_border: 'rgba(0, 0, 0, 0)',
  link_color: '#353fb5',
  cell_horizontal_padding: 8,
  cell_vertical_padding: 3,
  header_font_style: '600 13px',
  base_font_style: '13px',
  font_family: 'Inter, -apple-system, BlinkMacSystemFont, avenir next, avenir, segoe ui, helvetica neue, helvetica, Ubuntu, noto, arial, sans-serif',
  line_height: 1.4,
}

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const gridRef = useRef<WasmDataGrid | null>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const grid = new WasmDataGrid(canvas, {
      columns,
      rows: NUM_ROWS,
      getCellContent,
      headerHeight: 36,
      rowHeight: 34,
      freezeColumns: 2,
      theme,
    })

    grid.init().then(() => {
      console.log('WASM DataGrid initialized')
    }).catch(err => {
      console.error('WASM DataGrid init failed:', err)
    })

    gridRef.current = grid

    return () => {
      grid.destroy()
    }
  }, [])

  return (
    <div className="grid-container">
      <div className="grid-header">
        <h1>grid-wasm <span style={{ color: '#7c7cfc' }}>Rust + WASM</span></h1>
        <p>{NUM_ROWS.toLocaleString()} rows × {NUM_COLS} columns · Click to select · Arrow keys · Scroll to browse</p>
      </div>
      <canvas ref={canvasRef} />
    </div>
  )
}

export default App
