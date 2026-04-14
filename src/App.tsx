import { useEffect, useRef } from 'react'
import { WasmDataGrid, type ColumnInput } from './grid'
import './App.css'

type ExampleRow = {
  id: number,
  name: string,
  group: number,
  price: number,
  quantity: number,
  revenue: number,
  cost: number,
  active: boolean,
  // ISO 8601 datetime string — spans ~2.7 years at 1-hour intervals for 100k rows
  created_at: string,
}

// 100k rows at 1-hour intervals starting 2022-01-01 spans ~11.4 years
const EPOCH_START = new Date('2022-01-01T00:00:00Z').getTime()

function generateData(numRows: number): ExampleRow[] {
  const words = ['alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta', 'theta', 'kappa', 'sigma', 'omega']
  const data: ExampleRow[] = []

  for (let i = 0; i < numRows; i++) {
    const price = ((i * 31 + 7) % 10000) / 100
    const quantity = (i * 17 + 31) % 10000
    // One row per hour; 100k rows ≈ 11.4 years of hourly data
    const ts = new Date(EPOCH_START + i * 3_600_000)
    data.push({
      id: i,
      name: `${String.fromCharCode(65 + (i % 26))}-${words[(i * 7) % words.length]}-${i}`,
      group: i % 100,
      price,
      quantity,
      revenue: price * quantity,
      cost: price * quantity * (0.3 + (i % 10) / 20),
      active: i % 3 !== 0,
      created_at: ts.toISOString(),
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
        // dataStyle: {
        //   numberFormat: { type: 'currency', symbol: '$', decimals: 2 },
        //   align: 'right',
        // },
      },
            {
        name: 'group',
        display: 'Group',
        initWidth: 120,
      },
      {
        name: 'quantity',
        display: 'Qty',
        initWidth: 100,
        // dataStyle: {
        //   numberFormat: { type: 'integer' },
        //   align: 'right',
        //   conditionalFormats: [
        //     { type: 'greaterThan', value: 100, style: { bgColor: '#C8E6C9', color: '#1B5E20' } },
        //     { type: 'lessThan', value: 100, style: { bgColor: '#FFCDD2', color: '#B71C1C' } },
        //   ],
        // },
      },
      {
        name: 'revenue',
        display: 'Revenue',
        initWidth: 140,
        // dataStyle: {
        //   numberFormat: { type: 'accounting', decimals: 2 },
        //   align: 'right',
        //   conditionalFormats: [
        //     { type: 'greaterThan', value: 100000, style: { color: '#1B5E20', font: '700 13px Inter, sans-serif' } },
        //   ],
        // },
      },
      {
        name: 'cost',
        display: 'Cost',
        initWidth: 130,
        // dataStyle: {
        //   numberFormat: { type: 'accounting', decimals: 2 },
        //   align: 'right',
        // },
      },
    ],
  },
  {display:"",
    children:[{
    name: 'active',
    display: 'Active',
    initWidth: 80,
    // dataStyle: { align: 'center' },
  },]
  }
]

const columns2: ColumnInput[] = [
  { display: "Descriptions", children: [
    { name: 'id',   display: 'ID',           initWidth: 80,  isResizable: false, aggFunc: ['min'] },
    { name: 'name', display: 'Product Name', initWidth: 200, aggFunc: ['min'] },
  ]},
  { display: 'Financials', children: [
    { name: 'price',    display: 'Price',   initWidth: 120, aggFunc: ['mean', 'sum'] },
    { name: 'quantity', display: 'Qty',     initWidth: 100, aggFunc: ['min'] },
    { name: 'revenue',  display: 'Revenue', initWidth: 140, aggFunc: ['count', 'sum'] },
    { name: 'cost',     display: 'Cost',    initWidth: 130, aggFunc: ['count', 'sum'] },
  ]},
  { display: 'Time', children: [
    {
      name: 'created_at',
      display: 'Created At',
      initWidth: 160,
      // Group by month — the truncation submenu in ⋮ will show Year/Quarter/Month/Week/Day/Hour
      groupBy: 0,
      groupByTruncation: 'month',
      aggFunc: ['min', 'max'],
    },
  ]},
  { display: "", children: [
    { name: 'active', display: 'Active', initWidth: 80 },
  ]},
];

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
      columns: columns2,
      headerHeight: 60,
      rowHeight: 34,
      freezeColumns: 1,
    })

    grid.init().then(() => {
      console.log('WASM DataGrid initialized')
      console.log('Schema:', grid.getSchema())
      console.log('Rows:', grid.getRowCount())
      // Expose for Playwright tests
      ;(window as any).__gridDebug = (grid as any).grid
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
        <h1>grid-wasm <span style={{ color: '#7c7cfc' }}>Grouping · Date Truncation · Conditional Formatting</span></h1>
        <p>100k rows (hourly 2022–2033) · Grouped by Created At (Month) · Click ⋮ on date column to change truncation · Financials aggregations</p>
      </div>
      <canvas ref={canvasRef} />
    </div>
  )
}

export default App
