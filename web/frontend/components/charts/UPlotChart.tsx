'use client'

/**
 * Headless uPlot React wrapper.
 *
 * Key design decisions:
 * - Options are created once at mount (stable via useMemo in the caller).
 *   Data updates call u.setData() in-place — no re-mount, no React reconciliation.
 * - Width is managed internally via ResizeObserver; callers only set height in options.
 * - Zoom selection fires the onZoom callback (no auto-scale); callers call
 *   the imperative setScale/resetZoom handle to update the view after fetching data.
 * - Must be loaded with dynamic(..., { ssr: false }) since uPlot requires document.
 */

import { useEffect, useRef, forwardRef, useImperativeHandle } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'

export interface UPlotChartHandle {
  /** Reset x-axis view to show all data in the current dataset. */
  resetZoom: () => void
  /** Programmatically zoom the x-axis view to a specific range. */
  setScale: (min: number, max: number) => void
}

export interface UPlotChartProps {
  /** Columnar data: [timestamps, ...series]. Must be sorted ascending by timestamp. */
  data: uPlot.AlignedData
  /**
   * Chart options — should be a stable reference (memo'd by the caller).
   * Do NOT include `width`; the wrapper reads it from the container via ResizeObserver.
   */
  options: uPlot.Options
  /**
   * Called after scroll-zoom or drag-pan settles (400 ms debounce).
   * Receives the visible x-axis range so callers can fetch higher-resolution data.
   */
  onZoom?: (min: number, max: number) => void
  className?: string
  style?: React.CSSProperties
}

const UPlotChart = forwardRef<UPlotChartHandle, UPlotChartProps>(
  function UPlotChart({ data, options, onZoom, className, style }, ref) {
    const containerRef = useRef<HTMLDivElement>(null)
    const uplotRef = useRef<uPlot | null>(null)
    const onZoomRef = useRef(onZoom)
    // Tracks whether the user has panned/zoomed away from the auto-fit view.
    // When true, setData restores the saved viewport so live data doesn't jump back.
    // When false, setData lets uPlot auto-fit so new points are always visible.
    const isUserZoomedRef = useRef(false)
    const savedScaleRef = useRef<{ min: number; max: number } | null>(null)
    onZoomRef.current = onZoom

    useImperativeHandle(ref, () => ({
      resetZoom: () => {
        const u = uplotRef.current
        if (!u) return
        isUserZoomedRef.current = false
        savedScaleRef.current = null
        const ts = u.data[0] as number[]
        if (ts.length > 0) u.setScale('x', { min: ts[0], max: ts[ts.length - 1] })
        u.setSelect({ left: 0, top: 0, width: 0, height: 0 }, false)
      },
      setScale: (min: number, max: number) => {
        isUserZoomedRef.current = true
        savedScaleRef.current = { min, max }
        uplotRef.current?.setScale('x', { min, max })
      },
    }))

    // Mount / options change: create a new uPlot instance
    useEffect(() => {
      if (!containerRef.current) return

      const width = containerRef.current.clientWidth || 800

      // Spread options first, then override width so the container drives it
      const { width: _ignored, ...restOptions } = options as uPlot.Options & { width?: number }
      const opts: uPlot.Options = {
        ...restOptions,
        width,
        cursor: {
          ...options.cursor,
          drag: { x: false, y: false, setScale: false },
        },
        hooks: { ...options.hooks },
      }

      const u = new uPlot(opts, data, containerRef.current)
      uplotRef.current = u
      // Grab cursor hints that the chart is pannable
      u.over.style.cursor = 'grab'
      // Cache the overlay bounding rect — read once, reuse in wheel/pan handlers.
      // Updated in ResizeObserver so layout shifts after resize are accounted for.
      let overRect = u.over.getBoundingClientRect()

      // ── Shared helpers ──────────────────────────────────────────────────────

      // RAF batching: coalesce rapid scale mutations into one canvas redraw per frame.
      // Both wheel and pan events call scheduleScale(); the actual setScale fires once
      // per animation frame regardless of how many events arrive in that frame.
      let rafId: number | null = null
      let pendingMin: number | null = null
      let pendingMax: number | null = null
      const scheduleScale = (min: number, max: number) => {
        pendingMin = min
        pendingMax = max
        // Keep savedScaleRef current so the setData effect can restore the viewport.
        savedScaleRef.current = { min, max }
        if (!rafId) {
          rafId = requestAnimationFrame(() => {
            rafId = null
            if (pendingMin !== null && pendingMax !== null) {
              uplotRef.current?.setScale('x', { min: pendingMin, max: pendingMax })
              pendingMin = null
              pendingMax = null
            }
          })
        }
      }

      // Single shared debounce: fires onZoom after interactions settle (400 ms).
      // Both zoom and pan share this so a pan immediately following a zoom doesn't
      // trigger two backend fetches.
      let fetchDebounce: ReturnType<typeof setTimeout> | null = null
      const scheduleZoom = () => {
        if (fetchDebounce) clearTimeout(fetchDebounce)
        fetchDebounce = setTimeout(() => {
          const u = uplotRef.current
          if (!u || !onZoomRef.current) return
          const min = u.scales.x.min!
          const max = u.scales.x.max!
          if (Number.isFinite(min) && Number.isFinite(max) && max > min) {
            onZoomRef.current(min, max)
          }
        }, 400)
      }

      // ── Scroll-to-zoom ──────────────────────────────────────────────────────
      // Uses Math.exp for a smooth, continuous factor that works equally well for
      // discrete mouse wheels (large deltaY, few events) and trackpads (small
      // deltaY, many rapid events). Clamped to ±150 px to avoid jumps on fast swipes.
      const handleWheel = (e: WheelEvent) => {
        e.preventDefault()
        const u = uplotRef.current
        if (!u) return
        isUserZoomedRef.current = true

        const xMin = u.scales.x.min ?? 0
        const xMax = u.scales.x.max ?? 1
        const range = xMax - xMin
        if (range === 0) return

        // Normalize: line-mode devices (deltaMode=1) report lines, not pixels
        const rawDelta = e.deltaMode === 1 ? e.deltaY * 15 : e.deltaY
        const factor = Math.exp(Math.max(-150, Math.min(150, rawDelta)) * 0.003)

        // Keep the timestamp under the cursor fixed (focal-point zoom)
        const xPct = Math.max(0, Math.min(1, (e.clientX - overRect.left) / overRect.width))
        const xFocal = xMin + range * xPct
        const newRange = range * factor

        scheduleScale(xFocal - newRange * xPct, xFocal + newRange * (1 - xPct))
        scheduleZoom()
      }
      u.over.addEventListener('wheel', handleWheel, { passive: false })

      // ── Click-drag pan ──────────────────────────────────────────────────────
      // Mousedown records the anchor; subsequent mousemove events on the document
      // (so dragging outside the chart still works) translate pixel delta → data delta.
      let isPanning = false
      let panStartX = 0
      let panStartMin = 0
      let panStartMax = 0
      let panRectWidth = 0

      const handleMouseDown = (e: MouseEvent) => {
        if (e.button !== 0) return
        isPanning = true
        panStartX = e.clientX
        panStartMin = u.scales.x.min ?? 0
        panStartMax = u.scales.x.max ?? 1
        // Measure once at drag-start — rect doesn't change during a drag
        panRectWidth = u.over.getBoundingClientRect().width
        u.over.style.cursor = 'grabbing'
        document.body.style.cursor = 'grabbing'
        // Stop uPlot's own mousedown so it doesn't start a drag-select internally
        e.stopPropagation()
      }

      const handleMouseMove = (e: MouseEvent) => {
        if (!isPanning) return
        const range = panStartMax - panStartMin
        // Negate: dragging right moves the window left (shows earlier data), same as W&B
        const deltaData = -(e.clientX - panStartX) / panRectWidth * range
        scheduleScale(panStartMin + deltaData, panStartMax + deltaData)
      }

      const handleMouseUp = () => {
        if (!isPanning) return
        isPanning = false
        isUserZoomedRef.current = true
        u.over.style.cursor = 'grab'
        document.body.style.cursor = ''
        scheduleZoom()
      }

      u.over.addEventListener('mousedown', handleMouseDown)
      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)

      // ── Resize ──────────────────────────────────────────────────────────────
      const ro = new ResizeObserver(() => {
        const w = containerRef.current?.clientWidth
        if (w && w !== u.width) {
          u.setSize({ width: w, height: opts.height as number })
          overRect = u.over.getBoundingClientRect()
        }
      })
      ro.observe(containerRef.current)

      return () => {
        if (rafId) cancelAnimationFrame(rafId)
        if (fetchDebounce) clearTimeout(fetchDebounce)
        u.over.removeEventListener('wheel', handleWheel)
        u.over.removeEventListener('mousedown', handleMouseDown)
        document.removeEventListener('mousemove', handleMouseMove)
        document.removeEventListener('mouseup', handleMouseUp)
        ro.disconnect()
        u.destroy()
        uplotRef.current = null
      }
      // options identity drives recreation — callers must useMemo their options
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [options])

    // Data change: update in-place without re-mounting (sub-millisecond).
    // - Not zoomed (isUserZoomedRef=false): setData(true) auto-extends x-axis so new points
    //   are always visible in live runs.
    // - Zoomed/panned (isUserZoomedRef=true): setData(true) then immediately restore the saved
    //   viewport so the user's view doesn't jump. Both commits happen synchronously → one paint.
    useEffect(() => {
      const u = uplotRef.current
      if (!u) return
      u.setData(data as uPlot.AlignedData, true)
      if (isUserZoomedRef.current && savedScaleRef.current) {
        u.setScale('x', { min: savedScaleRef.current.min, max: savedScaleRef.current.max })
      }
    }, [data])

    return <div ref={containerRef} className={className} style={style} />
  }
)

export default UPlotChart
