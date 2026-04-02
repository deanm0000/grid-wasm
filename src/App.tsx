import { useEffect, useRef } from 'react'
import { WasmDataGrid, type ColumnInput } from './grid'
import './App.css'

type ExampleRow = {
  id: number,
  name: string,
  price: number,
  quantity: number,
  revenue: number,
  cost: number,
  active: boolean
}

function generateData(numRows: number): ExampleRow[] {
  const words = ['alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta', 'theta', 'kappa', 'sigma', 'omega']
  const data: ExampleRow[] = []

  for (let i = 0; i < numRows; i++) {
    const price = ((i * 31 + 7) % 10000) / 100
    const quantity = (i * 17 + 31) % 10000
    data.push({
      id: i,
      name: `${String.fromCharCode(65 + (i % 26))}-${words[(i * 7) % words.length]}-${i}`,
      price,
      quantity,
      revenue: price * quantity,
      cost: price * quantity * (0.3 + (i % 10) / 20),
      active: i % 3 !== 0,
    })
  }

  return data
}

const columns: ColumnInput[] = [
  {
    display: "Descriptions",
    headerStyle: { bgColor: '#E8EAF6', color: '#283593' },
    children: [
      {
        name: 'id',
        display: 'ID',
        initWidth: 80,
        isResizable: false,
      },
      {
        name: 'name',
        display: 'Product Name',
        initWidth: 200,
        headerStyle: { font: '700 14px Inter, sans-serif' },
      },
    ]
  },

  {
    display: 'Financials',
    headerStyle: { bgColor: '#E8EAF6', color: '#283593' },
    children: [
      {
        name: 'price',
        display: 'Price',
        initWidth: 120,
        dataStyle: {
          numberFormat: { type: 'currency', symbol: '$', decimals: 2 },
          align: 'right',
        },
      },
      {
        name: 'quantity',
        display: 'Qty',
        initWidth: 100,
        dataStyle: {
          numberFormat: { type: 'integer' },
          align: 'right',
          conditionalFormats: [
            { type: 'greaterThan', value: 100, style: { bgColor: '#C8E6C9', color: '#1B5E20' } },
            { type: 'lessThan', value: 100, style: { bgColor: '#FFCDD2', color: '#B71C1C' } },
          ],
        },
      },
      {
        name: 'revenue',
        display: 'Revenue',
        initWidth: 140,
        dataStyle: {
          numberFormat: { type: 'accounting', decimals: 2 },
          align: 'right',
          conditionalFormats: [
            { type: 'greaterThan', value: 100000, style: { color: '#1B5E20', font: '700 13px Inter, sans-serif' } },
          ],
        },
      },
      {
        name: 'cost',
        display: 'Cost',
        initWidth: 130,
        dataStyle: {
          numberFormat: { type: 'accounting', decimals: 2 },
          align: 'right',
        },
      },
    ],
  },
  {display:"",
    children:[{
    name: 'active',
    display: 'Active',
    initWidth: 80,
    dataStyle: { align: 'center' },
  },]
  }
  
]

function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const gridRef = useRef<WasmDataGrid | null>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const grid = new WasmDataGrid(canvas, {
      data: {
        kind: 'objects',
        data: generateData(100_000),
      },
      columns,
      headerHeight: 60,
      rowHeight: 34,
      freezeColumns: 1,
    })

    grid.init().then(() => {
      console.log('WASM DataGrid initialized')
      console.log('Schema:', grid.getSchema())
      console.log('Rows:', grid.getRowCount())
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
        <h1>grid-wasm <span style={{ color: '#7c7cfc' }}>Nested Headers · Custom Styles · Conditional Formatting</span></h1>
        <p>100k rows · Nested "Financials" header group · Accounting/Currency formats · Conditional colors on Qty/Revenue · Drag column borders to resize</p>
      </div>
      <canvas ref={canvasRef} />
    </div>
  )
}

export default App
