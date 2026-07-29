import { useMemo } from 'react'

interface TrendChartProps {
  data: { time: number; value: number }[]
  value: number
  height?: number
}

export function TrendChart({ data, value, height = 250 }: TrendChartProps) {
  const maxVal = useMemo(() => Math.max(...data.map(d => d.value), 1), [data])
  const svgW = data.length > 1 ? data.length * 6 : 100

  // 生成折线路径
  const pathD = useMemo(() => {
    if (data.length < 2) return ''
    const w = svgW
    const h = height - 40
    const pts = data.map((d, i) => {
      const x = (i / (data.length - 1)) * w
      const y = h - (d.value / maxVal) * h
      return `${i === 0 ? 'M' : 'L'}${x},${y}`
    })
    return pts.join(' ')
  }, [data, maxVal, svgW, height])

  // 填充区域路径
  const areaD = useMemo(() => {
    if (data.length < 2) return ''
    const w = svgW
    const h = height - 40
    const top = data.map((d, i) => {
      const x = (i / (data.length - 1)) * w
      const y = h - (d.value / maxVal) * h
      return `${i === 0 ? 'L' : 'L'}${x},${y}`
    }).join(' ')
    return `M0,${h} ${top} L${w},${h} Z`
  }, [data, maxVal, svgW, height])

  if (data.length < 2) {
    return (
      <div style={{ height }} className="flex items-center justify-center text-gray-400 text-sm">
        等待数据...
      </div>
    )
  }

  return (
    <div style={{ height }} className="relative">
      <svg
        viewBox={`0 0 ${svgW} ${height}`}
        className="w-full h-full"
        preserveAspectRatio="none"
      >
        {/* 填充区域 */}
        <path d={areaD} fill="oklch(0.623 0.214 255 / 0.12)" />

        {/* 折线 */}
        <path
          d={pathD}
          fill="none"
          stroke="oklch(0.623 0.214 255)"
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {/* 最新值圆点 */}
        <circle
          cx={svgW}
          cy={height - 40 - (data[data.length - 1].value / maxVal) * (height - 40)}
          r={4}
          fill="oklch(0.623 0.214 255)"
          stroke="#fff"
          strokeWidth={2}
        />

        {/* Y 轴标签 */}
        <text x={0} y={14} fontSize={10} fill="#999">{maxVal.toLocaleString()}</text>
        <text x={0} y={height - 8} fontSize={10} fill="#999">0</text>
      </svg>

      {/* 最新值浮标 */}
      <div className="absolute top-2 right-2 bg-white/90 rounded-lg px-2.5 py-1 shadow-sm text-xs tabular-nums">
        {value.toLocaleString()} 键
      </div>
    </div>
  )
}
