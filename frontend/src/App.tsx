import { useState, useEffect, useCallback, useMemo } from 'react'
import { AreaChart, Area } from './components/charts/area-chart'
import { Grid } from './components/charts/grid'
import { XAxis } from './components/charts/x-axis'
import { ChartTooltip } from './components/charts/tooltip/chart-tooltip'
import { LiveYAxis } from './components/charts/live-y-axis'
import { hourFmt } from './components/charts/chart-formatters'
import { KeyboardHeatmap } from './components/KeyboardHeatmap'
import { ScreenHeatmap } from './components/ScreenHeatmap'
import { KEYBOARD_LAYOUT } from './keyboard-layout'
import './index.css'

const KW = 32, MARGIN = 4

function getKeyboardWidth(): number {
  let minCol = Infinity, maxColEnd = 0
  for (const k of KEYBOARD_LAYOUT) {
    if (k.col < minCol) minCol = k.col
    if (k.col + k.w > maxColEnd) maxColEnd = k.col + k.w
  }
  return Math.round((maxColEnd - minCol) * KW + MARGIN * 2)
}

const keyWidth = getKeyboardWidth()

type TimeRange = '1d' | '7d' | 'all'

function getDateRange(range: TimeRange): { from: string; to: string } {
  const now = new Date()
  const to = now.toISOString().slice(0, 10)
  if (range === '1d') return { from: to, to }
  if (range === 'all') {
    return { from: '2026-01-01', to }
  }
  const from = new Date()
  from.setDate(from.getDate() - 6)
  return { from: from.toISOString().slice(0, 10), to }
}

function buildTrendData(
  raw: ({ hour?: number; date?: string; count?: number; value?: number })[],
  type: 'hourly' | 'daily' | 'daily_sparse' | 'monthly' | 'yearly',
): { date: Date; value: number }[] {
  const map = new Map<string, number>()
  for (const r of raw) {
    const key = r.date ?? String(r.hour ?? '')
    const v = r.value ?? r.count ?? 0
    map.set(key, (map.get(key) || 0) + v)
  }
  if (type === 'hourly') {
    const ts = new Date()
    ts.setHours(0, 0, 0, 0)
    return Array.from({ length: 24 }, (_, h) => {
      ts.setHours(h, 0, 0, 0)
      // 匹配 "0"、"00"、1、"01" 等格式
      return { date: new Date(ts), value: map.get(String(h)) || map.get(String(h).padStart(2, '0')) || 0 }
    })
  }
  if (type === 'monthly' || type === 'yearly') {
    return Array.from(map.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, count]) => ({ date: new Date(key + 'T00:00:00'), value: count }))
  }
  if (type === 'daily_sparse') {
    const entries = Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b))
    if (entries.length === 0) return []
    const first = new Date(entries[0][0] + 'T00:00:00')
    const last = new Date(entries[entries.length - 1][0] + 'T00:00:00')
    const pts: { date: Date; value: number }[] = []
    for (let d = new Date(first); d <= last; d.setDate(d.getDate() + 1)) {
      const key = d.toISOString().slice(0, 10)
      pts.push({ date: new Date(d), value: map.get(key) || 0 })
    }
    return pts
  }
  // daily with fill (7d mode)
  const start = new Date()
  start.setDate(start.getDate() - 6)
  const pts: { date: Date; value: number }[] = []
  for (let d = new Date(start), i = 0; i < 7; i++, d.setDate(d.getDate() + 1)) {
    const key = d.toISOString().slice(0, 10)
    pts.push({ date: new Date(d), value: map.get(key) || 0 })
  }
  return pts
}

const monthFmt = new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: 'short' })
const yearFmt = (d: Date) => d.getFullYear().toString()

interface RangeStats {
  from: string; to: string; total_keys: number; active_ms: number
  active_secs: number; wpm: number; mouse_moves: number; mouse_clicks: number
}
interface Info { version: string; uptime_secs: number; uptime_str: string }
interface MousePoint { x: number; y: number }

function TimeRangePicker({ value, onChange }: { value: TimeRange; onChange: (r: TimeRange) => void }) {
  const ranges: { key: TimeRange; label: string }[] = [
    { key: '1d', label: '1 天' }, { key: '7d', label: '7 天' }, { key: 'all', label: '所有' },
  ]
  return (
    <div className="time-range-picker">
      {ranges.map(r => (
        <button key={r.key} className={`time-range-btn ${value === r.key ? 'active' : ''}`}
          onClick={() => onChange(r.key)}>{r.label}</button>
      ))}
    </div>
  )
}

function KpiCard({ label, value, unit, accent, delay }: {
  label: string; value: string | number; unit?: string; accent: string; delay?: number
}) {
  return (
    <div className="kpi-card" style={{ animationDelay: `${delay ?? 0}ms` }}>
      <div className="kpi-accent" style={{ background: accent }} />
      <div className="kpi-body">
        <div className="kpi-label">{label}</div>
        <div className="kpi-row">
          <span className="kpi-value" style={{ color: accent }}>
            {typeof value === 'number' ? value.toLocaleString() : value}
          </span>
          {unit && <span className="kpi-unit">{unit}</span>}
        </div>
      </div>
    </div>
  )
}

type TrendMetric = 'keys' | 'wpm' | 'mouse_moves' | 'mouse_clicks'

const METRICS: Record<TrendMetric, { label: string; accent: string; meta: (s: RangeStats | null) => string }> = {
  keys:       { label: '累计按键', accent: '#3b82f6', meta: s => (s?.total_keys?.toLocaleString() ?? '—') + ' 键' },
  wpm:        { label: '手速',     accent: '#06b6d4', meta: s => (s?.wpm != null ? s.wpm.toFixed(1) : '—') + ' 键/分' },
  mouse_moves:{ label: '鼠标移动', accent: '#10b981', meta: s => (s?.mouse_moves?.toLocaleString() ?? '—') + ' 次' },
  mouse_clicks:{label: '鼠标点击', accent: '#14b8a6', meta: s => (s?.mouse_clicks?.toLocaleString() ?? '—') + ' 次' },
}

function App() {
  const [info, setInfo] = useState<Info | null>(null)
  const [range, setRange] = useState<TimeRange>('1d')
  const [metric, setMetric] = useState<TrendMetric>('keys')
  const [stats, setStats] = useState<RangeStats | null>(null)
  const [keyStats, setKeyStats] = useState<Record<number, number>>({})
  const [mousePoints, setMousePoints] = useState<MousePoint[]>([])
  const [error, setError] = useState<string | null>(null)
  const [trendData, setTrendData] = useState<{ date: Date; value: number }[]>([])
  const [trendType, setTrendType] = useState<'hourly' | 'daily' | 'monthly' | 'yearly'>('hourly')

  const dates = useMemo(() => getDateRange(range), [range])

  // 横坐标和 tooltip 的日期格式化
  const labelFormatter = useMemo(() => {
    if (trendType === 'hourly') return hourFmt.format
    if (trendType === 'monthly') return (d: Date) => monthFmt.format(d)
    if (trendType === 'yearly') return (d: Date) => yearFmt(d)
    return undefined
  }, [trendType])
  const dateFormatter = useMemo(() => labelFormatter, [labelFormatter])

  const fetchAll = useCallback(async () => {
    try {
      const [infoRes, statsRes]: [Info, RangeStats] = await Promise.all([
        fetch('/api/info').then(r => r.json()),
        fetch(`/api/stats/range?from=${dates.from}&to=${dates.to}`).then(r => r.json()),
      ])
      setInfo(infoRes)
      setStats(statsRes)
      setError(null)
    } catch { setError('无法连接到 keytrace 服务') }
  }, [dates])

  const fetchKeys = useCallback(async () => {
    try {
      const keysRes = await fetch(`/api/stats/keys?from=${dates.from}&to=${dates.to}`)
      if (keysRes.ok) {
        const keysData = await keysRes.json() as { keys: { key_code: number; count: number }[] }
        if (keysData.keys) {
          const s: Record<number, number> = {}
          for (const k of keysData.keys) s[k.key_code] = (s[k.key_code] || 0) + k.count
          setKeyStats(s)
        }
      }
    } catch { /* ignore */ }
  }, [dates])

  const fetchTrendAndMouse = useCallback(async () => {
    try {
      // 趋势图
      if (range === '7d' && metric === 'keys') {
        const res = await fetch('/api/stats/daily?days=7')
        if (res.ok) {
          const data = await res.json() as { days: { date: string; count: number }[] }
          setTrendData(buildTrendData(data.days || [], 'daily'))
          setTrendType('daily')
        }
      } else {
        const res = await fetch(`/api/stats/trend?from=${dates.from}&to=${dates.to}&metric=${metric}`)
        if (res.ok) {
          const data = await res.json() as { type: string; points: { date: string; value: number }[] }
          const t = data.type as 'hourly' | 'daily' | 'monthly' | 'yearly'
          setTrendData(buildTrendData(data.points || [], t === 'daily' ? 'daily_sparse' : t))
          setTrendType(t)
        }
      }
      // 鼠标热力图
      const fromTs = `${dates.from}T00:00:00`
      const toTs = `${dates.to}T23:59:59`
      const res = await fetch(`/api/mouse/moves?from=${fromTs}&to=${toTs}&limit=8000`)
      if (res.ok) {
        const data = await res.json() as { moves: { x: number; y: number }[] }
        if (data.moves) setMousePoints(data.moves.map(m => ({ x: m.x, y: m.y })))
      }
    } catch { /* ignore */ }
  }, [dates, range, metric])

  // KPI + 键盘热力图：3 秒快速轮询
  useEffect(() => {
    fetchAll(); fetchKeys()
    const t = setInterval(() => { fetchAll(); fetchKeys() }, 3000)
    return () => clearInterval(t)
  }, [fetchAll, fetchKeys])

  // 趋势图 + 鼠标热力图：15 秒慢轮询
  useEffect(() => {
    fetchTrendAndMouse()
    const t = setInterval(fetchTrendAndMouse, 15000)
    return () => clearInterval(t)
  }, [fetchTrendAndMouse])

  return (
    <div className="app-root">
      <header className="app-header">
        <div className="app-header-inner">
          <div className="app-header-left">
            <div>
              <h1 className="app-title">KeyTrace</h1>
              <p className="app-subtitle">{info ? `运行 ${info.uptime_str}` : '连接中...'}</p>
            </div>
          </div>
          <div className="app-header-right">
            <TimeRangePicker value={range} onChange={setRange} />
            <span className="app-badge">v{info?.version ?? '—'}</span>
            <span className="app-dot" />
            <span className="app-status-text">实时监控中</span>
          </div>
        </div>
      </header>

      <main className="app-main">
        {error && <div className="app-error">{error}</div>}

        <div className="kpi-grid">
          <KpiCard label="累计按键" value={stats?.total_keys ?? 0} unit="键" accent={METRICS.keys.accent} delay={0} />
          <KpiCard label="手速" value={stats?.wpm != null ? stats.wpm.toFixed(1) : '—'} unit="键/分" accent={METRICS.wpm.accent} delay={60} />
          <KpiCard label="鼠标移动" value={stats?.mouse_moves?.toLocaleString() ?? '—'} unit="次" accent={METRICS.mouse_moves.accent} delay={120} />
          <KpiCard label="鼠标点击" value={stats?.mouse_clicks?.toLocaleString() ?? '—'} unit="次" accent={METRICS.mouse_clicks.accent} delay={180} />
        </div>

        <div className="card chart-card">
          <div className="card-header">
            <div className="card-tabs">
              {(Object.keys(METRICS) as TrendMetric[]).map(m => (
                <button
                  key={m}
                  className={`card-tab ${metric === m ? 'active' : ''}`}
                  style={metric === m ? { color: METRICS[m].accent, borderColor: METRICS[m].accent } : undefined}
                  onClick={() => setMetric(m)}
                >
                  {METRICS[m].label}
                </button>
              ))}
            </div>
            <span className="card-meta">{METRICS[metric].meta(stats)}</span>
          </div>
          <div className="chart-container">
            <AreaChart data={trendData} margin={{ left: 56, right: 16, top: 8, bottom: 32 }} labelFormatter={labelFormatter} aspectRatio="3.7/1">
              <Grid horizontal />
              <LiveYAxis formatValue={(v) => Math.max(0, Math.round(v)).toLocaleString()} allowDecimals={false} />
              <Area dataKey="value" fill={METRICS[metric].accent} fillOpacity={0.35} />
              <XAxis tickMode="data" numTicks={range === '1d' ? 6 : 7} />
              <ChartTooltip dateFormatter={dateFormatter} />
            </AreaChart>
          </div>
        </div>

        <div className="card heatmap-card">
          <div className="card-header">
            <span className="card-title">键盘热力图</span>
            <span className="card-meta">{Object.values(keyStats).reduce((a, b) => a + b, 0).toLocaleString()} 键</span>
          </div>
          <div className="heatmap-body"><KeyboardHeatmap keyStats={keyStats} /></div>
        </div>

        <div className="card heatmap-card">
          <div className="card-header"><span className="card-title">屏幕热力图</span></div>
          <div className="heatmap-body"><ScreenHeatmap points={mousePoints} targetWidth={keyWidth} /></div>
        </div>

        <div className="app-footer">数据每 15 秒刷新 · 最新 {new Date().toLocaleTimeString('zh-CN')}</div>
      </main>
    </div>
  )
}

export default App
