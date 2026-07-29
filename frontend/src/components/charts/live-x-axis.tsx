"use client";

import { motion, useSpring } from "motion/react";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useChart, useChartStable } from "./chart-context";
import { hmsTimeFmt } from "./chart-formatters";

const TICKER_HALF_WIDTH = 50;
const FADE_BUFFER = 20;

const crosshairSpringConfig = { stiffness: 300, damping: 30 };

function labelFadeOpacity(
  labelX: number,
  crosshairX: number | null,
  isHovering: boolean
): number {
  if (!isHovering || crosshairX === null) {
    return 1;
  }
  const distance = Math.abs(labelX - crosshairX);
  if (distance < TICKER_HALF_WIDTH) {
    return 0;
  }
  if (distance < TICKER_HALF_WIDTH + FADE_BUFFER) {
    return (distance - TICKER_HALF_WIDTH) / FADE_BUFFER;
  }
  return 1;
}

export interface LiveXAxisProps {
  /** Number of time labels. Default: 5 */
  numTicks?: number;
  /** Time formatter. Default: HH:MM:SS */
  formatTime?: (t: number) => string;
}

const defaultFormatTime = (t: number) => hmsTimeFmt.format(new Date(t));

export function LiveXAxis(props: LiveXAxisProps) {
  const { containerRef } = useChartStable();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const container = containerRef.current;
  if (!(mounted && container)) {
    return null;
  }

  return <LiveXAxisInner {...props} container={container} />;
}

const LiveXAxisInner = memo(function LiveXAxisInner({
  numTicks = 5,
  formatTime = defaultFormatTime,
  container,
}: LiveXAxisProps & { container: HTMLDivElement }) {
  const { xScale, margin, tooltipData } = useChart();

  const domain = xScale.domain();
  const startMs = domain[0]?.getTime() ?? 0;
  const endMs = domain[1]?.getTime() ?? 0;

  const labels = useMemo(() => {
    const step = (endMs - startMs) / (numTicks - 1);
    return Array.from({ length: numTicks }, (_, i) => {
      const t = startMs + i * step;
      const x = (xScale(new Date(t)) ?? 0) + margin.left;
      return { x, label: formatTime(t), stableKey: i };
    });
  }, [startMs, endMs, numTicks, xScale, margin.left, formatTime]);

  const isHovering = tooltipData !== null;
  const crosshairX = tooltipData ? tooltipData.x + margin.left : null;

  // Time pill label
  const pillLabel = useMemo(() => {
    if (!tooltipData) {
      return null;
    }
    const timeMs = xScale.invert(tooltipData.x).getTime();
    return formatTime(timeMs);
  }, [tooltipData, xScale, formatTime]);

  // Spring-animated pill position — matches TooltipIndicator's spring config
  // so the pill and crosshair line move in lockstep
  const pillX = tooltipData ? tooltipData.x + margin.left : 0;
  const animatedPillX = useSpring(pillX, crosshairSpringConfig);
  const springRef = useRef(animatedPillX);
  springRef.current = animatedPillX;

  useEffect(() => {
    springRef.current.set(pillX);
  }, [pillX]);

  return createPortal(
    <div style={{ position: 'absolute', top: 0, right: 0, bottom: 0, left: 0, pointerEvents: 'none' }}>
      {/* Time labels */}
      {labels.map((l) => (
        <div
          key={l.stableKey}
          style={{
            position: 'absolute',
            left: l.x,
            bottom: 12,
            width: 0,
            display: "flex",
            justifyContent: "center",
          }}
        >
          <motion.span
            animate={{
              opacity: labelFadeOpacity(l.x, crosshairX, isHovering),
            }}
            style={{ whiteSpace: 'nowrap', fontFamily: 'var(--chart-font-mono)', fontSize: 12, lineHeight: '16px', color: 'var(--chart-foreground)' }}
            transition={{ duration: 0.15, ease: "easeOut" }}
          >
            {l.label}
          </motion.span>
        </div>
      ))}

      {/* Time pill at crosshair — spring-animated to match crosshair line */}
      {isHovering && pillLabel && (
        <motion.div
          style={{
            position: 'absolute',
            zIndex: 50,
            left: animatedPillX,
            x: "-50%",
            bottom: 4,
          }}
        >
          <div style={{ overflow: 'hidden', borderRadius: 9999, backgroundColor: '#18181b', padding: '4px 16px', color: '#fff', boxShadow: '0 10px 15px -3px rgba(0,0,0,0.1)' }}>
            <span style={{ whiteSpace: 'nowrap', fontWeight: 500, fontSize: 14, lineHeight: '20px' }}>
              {pillLabel}
            </span>
          </div>
        </motion.div>
      )}
    </div>,
    container
  );
});

LiveXAxis.displayName = "LiveXAxis";

export default LiveXAxis;
