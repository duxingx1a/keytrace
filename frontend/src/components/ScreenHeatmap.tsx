import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { motion, AnimatePresence } from 'motion/react'

interface MousePoint { x: number; y: number }
interface ScreenInfo { left: number; top: number; right: number; bottom: number; width: number; height: number }
interface Props { points: MousePoint[]; targetWidth: number }

const JET: [number, number, number][] = [
  [80, 140, 230], [110, 165, 240], [145, 192, 248], [185, 215, 250],
  [220, 232, 250],
  [248, 247, 248],
  [248, 222, 195], [248, 204, 165], [238, 152, 105], [210, 90, 55], [170, 48, 32],
]

function heatColor(t: number): [number, number, number] {
  if (t <= 0) return JET[0]
  const maxI = JET.length - 1
  const idx = Math.min(Math.floor(t * maxI), maxI - 1)
  const frac = t * maxI - idx
  const a = JET[idx]; const b = JET[idx + 1]
  return [Math.round(a[0]+(b[0]-a[0])*frac), Math.round(a[1]+(b[1]-a[1])*frac), Math.round(a[2]+(b[2]-a[2])*frac)]
}

export function ScreenHeatmap({ points, targetWidth }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [screens, setScreens] = useState<ScreenInfo[]>([])
  const [tooltip, setTooltip] = useState<{ x: number; y: number; screenX: number; screenY: number; count: number } | null>(null)

  useEffect(() => {
    fetch('/api/info/screens').then(r => r.json()).then(d => {
      if (d.screens) setScreens(d.screens)
    }).catch(() => {})
  }, [])

  let minX = 0, minY = 0, totalW = 100, totalH = 100
  if (screens.length > 0) {
    minX = Math.min(...screens.map(s => s.left))
    minY = Math.min(...screens.map(s => s.top))
    const maxX = Math.max(...screens.map(s => s.right))
    const maxY = Math.max(...screens.map(s => s.bottom))
    totalW = maxX - minX
    totalH = maxY - minY
  }

  const scale = Math.min(targetWidth / totalW, 300 / totalH, 1)
  const cw = Math.round(totalW * scale)
  const ch = Math.round(totalH * scale)

  // 按 20px 网格分桶统计点密度
  const GRID = 20
  const gridCounts = useMemo(() => {
    const map = new Map<string, number>()
    for (const p of points) {
      const gx = Math.round(p.x / GRID)
      const gy = Math.round(p.y / GRID)
      const key = `${gx}_${gy}`
      map.set(key, (map.get(key) || 0) + 1)
    }
    return map
  }, [points])

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (screens.length === 0) return
    const rect = e.currentTarget.getBoundingClientRect()
    const cx = (e.clientX - rect.left) * (cw / rect.width)
    const cy = (e.clientY - rect.top) * (ch / rect.height)
    const sx = Math.round(cx / scale + minX)
    const sy = Math.round(cy / scale + minY)
    const inScreen = screens.some(
      s => sx >= s.left && sx < s.right && sy >= s.top && sy < s.bottom
    )
    if (!inScreen) { setTooltip(null); return }
    const gx = Math.round(sx / GRID)
    const gy = Math.round(sy / GRID)
    const count = gridCounts.get(`${gx}_${gy}`) || 0
    setTooltip({ x: e.clientX, y: e.clientY, screenX: sx, screenY: sy, count })
  }, [cw, ch, scale, minX, minY, screens, gridCounts])

  const handleMouseLeave = useCallback(() => setTooltip(null), [])

  useEffect(() => {
    const cvs = canvasRef.current
    if (!cvs || cw === 0 || ch === 0 || screens.length === 0) return
    const ctx = cvs.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    const pxW = cw * dpr
    const pxH = ch * dpr
    cvs.width = pxW
    cvs.height = pxH

    const densityCanvas = document.createElement('canvas')
    densityCanvas.width = pxW
    densityCanvas.height = pxH
    const dc = densityCanvas.getContext('2d')!
    dc.fillStyle = 'rgb(0,0,0)'
    dc.fillRect(0, 0, pxW, pxH)

    if (points.length > 0) {
      const dotR = 3 * dpr
      // 全部画，不降采样 — API limit 8000 已经控制了总量
      for (const p of points) {
        const cx = Math.round((p.x - minX) * scale * dpr)
        const cy = Math.round((p.y - minY) * scale * dpr)
        if (cx < -10 || cy < -10 || cx > pxW + 10 || cy > pxH + 10) continue
        if (!screens.some(s => p.x >= s.left && p.x < s.right && p.y >= s.top && p.y < s.bottom)) continue
        dc.fillStyle = 'rgba(255,255,255,0.15)'
        dc.beginPath(); dc.arc(cx, cy, dotR, 0, Math.PI * 2); dc.fill()
      }
    }

    // 第一轮模糊：把白点摊开成密度云
    dc.filter = `blur(${Math.round(14 * dpr)}px)`
    dc.drawImage(densityCanvas, 0, 0)
    dc.filter = 'none'

    // getImageData → 亮度→Jet着色
    const imgData = dc.getImageData(0, 0, pxW, pxH)
    const d = imgData.data
    let maxW = 0
    for (let i = 0; i < d.length; i += 4) { if (d[i] > maxW) maxW = d[i] }
    if (maxW < 2) maxW = 2

    for (let i = 0; i < d.length; i += 4) {
      const w = d[i]
      const px = ((i / 4) % pxW) / scale / dpr + minX
      const py = Math.floor(i / 4 / pxW) / scale / dpr + minY
      if (!screens.some(s => px >= s.left && px < s.right && py >= s.top && py < s.bottom)) {
        d[i] = 80; d[i + 1] = 140; d[i + 2] = 230; d[i + 3] = 255
        continue
      }
      const t = maxW > 0 ? Math.log1p(w) / Math.log1p(maxW) : 0
      const [r, g, b] = heatColor(t)
      d[i] = r; d[i + 1] = g; d[i + 2] = b; d[i + 3] = 255
    }
    dc.putImageData(imgData, 0, 0)

    // 第二轮小模糊：平滑 Jet 着色后的色块边缘，消除孤点
    const smoothCanvas = document.createElement('canvas')
    smoothCanvas.width = pxW; smoothCanvas.height = pxH
    const sc = smoothCanvas.getContext('2d')!
    sc.filter = `blur(${Math.round(1.5 * dpr)}px)`
    sc.drawImage(densityCanvas, 0, 0)
    sc.filter = 'none'

    ctx.drawImage(smoothCanvas, 0, 0)

    // ── 屏幕边框 ──
    for (const sc of screens) {
      const sx = Math.round((sc.left - minX) * scale)
      const sy = Math.round((sc.top - minY) * scale)
      ctx.strokeStyle = 'rgba(60,60,75,0.8)'; ctx.lineWidth = 1.5
      ctx.strokeRect(sx, sy, Math.round(sc.width * scale), Math.round(sc.height * scale))
      ctx.strokeStyle = 'rgba(255,255,255,0.2)'; ctx.lineWidth = 3
      ctx.strokeRect(sx, sy, Math.round(sc.width * scale), Math.round(sc.height * scale))
    }
  }, [points, screens, minX, minY, totalW, totalH, cw, ch, scale])

  return (
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center' }}>
      <div style={{
        width: cw,
        borderRadius: 12,
        overflow: 'hidden',
        boxShadow: 'inset 0 0 0 2px #76839B',
        background: '#76839B',
        lineHeight: 0,
      }}>
        <canvas
          ref={canvasRef}
          style={{ width: cw, height: ch, maxWidth: '100%', cursor: 'crosshair' }}
          onMouseMove={handleMouseMove}
          onMouseLeave={handleMouseLeave}
        />
      </div>
      {tooltip && (
        <AnimatePresence>
          <motion.div
            initial={{ opacity: 0, scale: 0.96, y: 4 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: 4 }}
            transition={{ type: 'spring', stiffness: 500, damping: 30 }}
            style={{
              position: 'fixed',
              left: tooltip.x + 14,
              top: tooltip.y - 10,
              zIndex: 999,
              pointerEvents: 'none',
            }}
          >
            <div style={{
              background: 'oklch(1 0 0)',
              borderRadius: 10,
              padding: '8px 14px',
              boxShadow: '0 4px 20px rgba(0,0,0,0.12), 0 0 0 0.5px rgba(0,0,0,0.06)',
              minWidth: 120,
            }}>
              <div style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: 12,
                marginBottom: 4,
              }}>
                <span style={{
                  color: 'oklch(0.55 0.014 260)',
                  fontSize: 13,
                  lineHeight: '20px',
                }}>{tooltip.screenX}, {tooltip.screenY}</span>
                <span style={{
                  color: 'oklch(0.145 0.004 285)',
                  fontSize: 13,
                  fontWeight: 600,
                  fontVariantNumeric: 'tabular-nums',
                  lineHeight: '20px',
                }}>{tooltip.count.toLocaleString()}</span>
              </div>
            </div>
          </motion.div>
        </AnimatePresence>
      )}
    </div>
  )
}
