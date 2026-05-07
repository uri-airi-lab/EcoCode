/**
 * uPlot shared helpers — no runtime uPlot import, safe to load on server.
 * uPlot types are imported as `import type` (erased at compile time).
 */
import { format } from 'date-fns'

export interface TooltipPluginOpts {
  /** Display labels for each data series (excluding x-axis). */
  labels: string[]
  /** CSS color strings matching the series strokes. */
  colors: string[]
  /** Custom x-value formatter. Defaults to 'HH:mm:ss' timestamp. */
  formatX?: (val: number) => string
  /** Custom y-value formatter. Defaults to two decimal places. */
  formatY?: (val: number, seriesIdx: number) => string
  /** Unit suffix displayed after y values (e.g. ' W', '%'). */
  unit?: string
}

/**
 * Returns a uPlot plugin that renders a floating tooltip following the cursor.
 * The tooltip is appended to `u.over` (the overlay div above the canvas).
 */
export function makeTooltipPlugin(opts: TooltipPluginOpts): { hooks: object } {
  const { labels, colors, formatX, formatY, unit = '' } = opts
  let tooltip: HTMLDivElement | null = null
  let lastIdx: number | null = null
  let cachedTtWidth = 160

  return {
    hooks: {
      init: (u: any) => {
        tooltip = document.createElement('div')
        tooltip.style.cssText = [
          'position:absolute',
          'background:hsl(222 47% 10%)',
          'border:1px solid hsl(217.2 32.6% 17.5%)',
          'border-radius:4px',
          'padding:8px 12px',
          'pointer-events:none',
          'display:none',
          'z-index:10',
          'font-size:11px',
          'font-family:var(--font-jetbrains-mono,monospace)',
          'color:hsl(215 20.2% 65.1%)',
          'white-space:nowrap',
          'min-width:130px',
        ].join(';')
        u.over.appendChild(tooltip)
      },

      setCursor: (u: any) => {
        if (!tooltip) return
        const { left, top, idx } = u.cursor
        if (idx == null || (left != null && left < 0)) {
          tooltip.style.display = 'none'
          lastIdx = null
          return
        }
        const ts = (u.data[0] as number[])?.[idx]
        if (ts == null || !Number.isFinite(ts)) {
          tooltip.style.display = 'none'
          lastIdx = null
          return
        }

        // Only rebuild HTML when the data index changes — skips string allocs + DOM
        // mutation + layout read on every cursor pixel move between data points.
        if (idx !== lastIdx) {
          lastIdx = idx
          const timeLabel = formatX ? formatX(ts) : format(new Date(ts), 'HH:mm:ss')
          let html = `<div style="font-weight:600;margin-bottom:6px;color:#e2e8f0">${timeLabel}</div>`
          for (let i = 1; i < u.data.length; i++) {
            const val = (u.data[i] as number[])?.[idx]
            if (val == null) continue
            const label = labels[i - 1] ?? `Series ${i}`
            const color = colors[i - 1] ?? '#888'
            const formatted = formatY ? formatY(val, i - 1) : val.toFixed(2)
            html += [
              '<div style="display:flex;justify-content:space-between;gap:14px;line-height:1.6">',
              `<span style="color:${color}">● ${label}</span>`,
              `<span>${formatted}${unit}</span>`,
              '</div>',
            ].join('')
          }
          tooltip.innerHTML = html
          tooltip.style.display = 'block'
          cachedTtWidth = tooltip.offsetWidth || 160
        }

        // Position update on every cursor move (two cheap style writes, no layout read)
        const overWidth = u.over.clientWidth
        const showLeft = (left ?? 0) + 15 + cachedTtWidth > overWidth
          ? (left ?? 0) - cachedTtWidth - 10
          : (left ?? 0) + 15
        tooltip.style.left = showLeft + 'px'
        tooltip.style.top = Math.max(0, (top ?? 0) - 20) + 'px'
      },

      // Hide tooltip when data changes (zoom / reset) and reset dirty-check
      setData: () => {
        if (tooltip) tooltip.style.display = 'none'
        lastIdx = null
      },
    },
  }
}

/** Shared axis style constants for a dark-theme uPlot chart. */
export const AXIS_STROKE = 'hsl(215 20.2% 65.1%)'
export const GRID_STROKE = 'rgba(255,255,255,0.05)'
export const AXIS_FONT = '10px sans-serif'
