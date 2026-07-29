import { useEffect, useMemo, useRef, useState, useCallback } from 'react'
import { motion, AnimatePresence } from 'motion/react'
import { KEYBOARD_LAYOUT } from '../keyboard-layout'

interface Props { keyStats: Record<number, number> }

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

const KW = 32; const KH = 32; const GAP = 3; const MARGIN = 4

export function KeyboardHeatmap({ keyStats }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const overlayRef = useRef<HTMLDivElement>(null)
  const [tooltip, setTooltip] = useState<{ x: number; y: number; label: string; count: number } | null>(null)
  const maxVal = Math.max(...Object.values(keyStats), 1)
  const logMax = useMemo(() => Math.log1p(maxVal), [maxVal])

  const { cw, ch, offsetX, offsetY } = useMemo(() => {
    let minCol = Infinity, maxColEnd = 0, maxRow = 0
    for (const k of KEYBOARD_LAYOUT) {
      if (k.col < minCol) minCol = k.col
      if (k.col + k.w > maxColEnd) maxColEnd = k.col + k.w
      if (k.row > maxRow) maxRow = k.row
    }
    return {
      cw: Math.round((maxColEnd - minCol) * KW + MARGIN * 2),
      ch: Math.round((maxRow + 1) * (KH + GAP) + MARGIN * 2),
      offsetX: -(minCol * KW - MARGIN),
      offsetY: MARGIN,
    }
  }, [])

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect()
    const mx = (e.clientX - rect.left) * (cw / rect.width)
    const my = (e.clientY - rect.top) * (ch / rect.height)
    for (const k of KEYBOARD_LAYOUT) {
      const kx = offsetX + k.col * KW
      const ky = offsetY + k.row * (KH + GAP)
      const kw = k.w * KW - GAP
      const kh = (k.h || 1) * KH + ((k.h || 1) - 1) * GAP - GAP
      if (mx >= kx && mx <= kx + kw && my >= ky && my <= ky + kh) {
        const count = keyStats[k.code] || 0
        setTooltip({ x: e.clientX, y: e.clientY, label: k.label || `VK${k.code}`, count })
        return
      }
    }
    setTooltip(null)
  }, [cw, ch, offsetX, offsetY, keyStats])

  const handleMouseLeave = useCallback(() => setTooltip(null), [])

  // 键帽覆盖层 — 颜色用 heatColor 数学计算（和 Canvas 完全相同）
  const overlays = useMemo(() => KEYBOARD_LAYOUT.map(k => {
    const count = keyStats[k.code] || 0
    const t = Math.log1p(count) / Math.max(logMax, 0.01)
    const [kr, kg, kb] = heatColor(t)
    return {
      uid: `${k.row}-${k.col}-${k.code}`,
      x: offsetX + k.col * KW,
      y: offsetY + k.row * (KH + GAP),
      w: k.w * KW - GAP,
      h: (k.h || 1) * KH + ((k.h || 1) - 1) * GAP - GAP,
      label: k.label, code: k.code, count, kr, kg, kb,
    }
  }), [offsetX, offsetY, keyStats, logMax])

  useEffect(() => {
    const cvs = canvasRef.current
    if (!cvs) return
    const ctx = cvs.getContext('2d', { willReadFrequently: true })
    if (!ctx) return
    const dpr = window.devicePixelRatio || 1
    const pxW = cw * dpr
    const pxH = ch * dpr
    cvs.width = pxW
    cvs.height = pxH

    const density = document.createElement('canvas')
    density.width = pxW
    density.height = pxH
    const dc = density.getContext('2d')!
    dc.fillStyle = 'rgb(80,140,230)'
    dc.fillRect(0, 0, pxW, pxH)

    for (const k of KEYBOARD_LAYOUT) {
      const count = keyStats[k.code] || 0
      if (count === 0) continue
      const intensity = Math.log1p(count) / logMax
      const x = Math.round((offsetX + k.col * KW) * dpr) + 1
      const y = Math.round((offsetY + k.row * (KH + GAP)) * dpr) + 1
      const w = Math.round((k.w * KW - GAP) * dpr) - 2
      const kh = (k.h || 1) * KH + ((k.h || 1) - 1) * GAP - GAP
      const h = Math.round(kh * dpr) - 2
      const a = 0.1 + intensity * 0.5
      dc.fillStyle = `rgba(255,255,255,${a})`
      dc.fillRect(x, y, Math.max(w, 1), Math.max(h, 1))
    }

    dc.filter = `blur(${Math.round(KW * 0.25 * dpr)}px)`
    dc.drawImage(density, 0, 0)
    dc.filter = 'none'

    // Jet 着色：亮度→热力色（蓝底+白覆盖→密度云→Jet映射）
    const imgData = dc.getImageData(0, 0, pxW, pxH)
    const d = imgData.data
    let maxW = 0, minW = 255
    for (let i = 0; i < d.length; i += 4) { if (d[i] > maxW) maxW = d[i]; if (d[i] < minW) minW = d[i] }
    const range = Math.max(maxW - minW, 1)

    for (let i = 0; i < d.length; i += 4) {
      const w = d[i]
      const t = (w - minW) / range
      const [r, g, b] = heatColor(t)
      d[i] = r; d[i+1] = g; d[i+2] = b; d[i+3] = 255
    }
    dc.putImageData(imgData, 0, 0)
    ctx.drawImage(density, 0, 0)

    for (const k of KEYBOARD_LAYOUT) {
      const x = offsetX + k.col * KW, y = offsetY + k.row * (KH + GAP)
      const w = k.w * KW - GAP, h = (k.h||1)*KH+((k.h||1)-1)*GAP - GAP
      ctx.strokeStyle = 'rgba(0,0,0,0.45)'; ctx.lineWidth = 2.2
      ctx.beginPath(); ctx.roundRect(x, y, w, h, 4); ctx.stroke()
      ctx.strokeStyle = 'rgba(255,255,255,0.35)'; ctx.lineWidth = 1.0
      ctx.beginPath(); ctx.roundRect(x, y, w, h, 4); ctx.stroke()
      if (!k.label) continue
      const count = keyStats[k.code]||0, t = Math.log1p(count)/logMax
      ctx.textAlign='center'; ctx.textBaseline='middle'
      ctx.fillStyle='rgba(0,0,0,0.55)'
      ctx.font='bold 10px Inter,sans-serif'
      ctx.fillText(k.label, x+w/2+1, y+h/2+1)
      ctx.fillStyle= t>0.25?'#fff':'rgba(255,255,255,0.85)'
      ctx.fillText(k.label, x+w/2, y+h/2)
    }
  }, [keyStats, maxVal, logMax, cw, ch, offsetX, offsetY])

  return (
    <div style={{ position: 'relative', width: cw, maxWidth: '100%', margin: '0 auto' }}>
      <canvas
        ref={canvasRef}
        style={{ display: 'block', width: '100%', height: ch, maxWidth: '100%', borderRadius: 8 }}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      />
      {/* 透明交互层 - CSS transition 弹起 */}
      <div ref={overlayRef} style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
        {overlays.map(o => (
          <div
            key={o.uid}
            className="keycap-hover"
            style={{
              position: 'absolute', left: o.x, top: o.y, width: o.w, height: o.h,
              borderRadius: 4, pointerEvents: 'auto', cursor: 'default',
              transformOrigin: 'center center',
              transition: 'transform 0.18s cubic-bezier(0.34,1.56,0.64,1), box-shadow 0.18s ease',
              '--kr': o.kr,
              '--kg': o.kg,
              '--kb': o.kb,
            } as React.CSSProperties}
            onMouseEnter={e => setTooltip({ x: e.clientX, y: e.clientY, label: o.label || `VK${o.code}`, count: o.count })}
            onMouseMove={e => setTooltip(p => p ? { ...p, x: e.clientX, y: e.clientY } : null)}
            onMouseLeave={() => setTooltip(null)}
          >
            <span className="keycap-hover-label">{o.label}</span>
          </div>
        ))}
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
              minWidth: 100,
            }}>
              <div style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: 12,
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span style={{
                    width: 9,
                    height: 9,
                    borderRadius: '50%',
                    flexShrink: 0,
                    background: `rgb(${heatColor(Math.log1p(Math.max(tooltip.count, 0)) / Math.max(logMax, 0.01))})`,
                  }} />
                  <span style={{
                    color: 'oklch(0.55 0.014 260)',
                    fontSize: 13,
                    lineHeight: '20px',
                  }}>{tooltip.label}</span>
                </div>
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
