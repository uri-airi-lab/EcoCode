# Implementation Plan: UI Redesign (W&B-Inspired)

## Overview

Replace the current dark glassmorphism aesthetic with a W&B-inspired design language: neutral charcoal background, vivid gold/amber primary accent, flat panels with thin 1px borders, and clean humanist sans-serif typography. All changes are purely visual — CSS tokens, Tailwind config, font imports, class swaps, and Recharts prop updates. No routing or data-fetching changes.

## Tasks

- [x] 1. Update design tokens in `globals.css`
  - Replace all `:root` HSL channel values per the token table in the design document (background, foreground, card, popover, primary, secondary, muted, accent, destructive, border, input, ring, chart-1 through chart-5, radius)
  - Add `--chart-6: 54 100% 73%` token
  - Add `--font-sans: var(--font-source-sans)` and `--font-mono: var(--font-jetbrains-mono)` tokens
  - Redefine `.metric-card` in `@layer components`: remove `backdrop-blur-lg`, `bg-black/20`, `border-white/10`, `rounded-xl`; replace with `bg-card border-border rounded-md` and `hover:border-primary/40`
  - Redefine `.chart-container`: same flat-panel treatment, remove blur and transparency
  - Redefine `.sidebar-glass`: `bg-card border-r border-border` (no blur, no transparency)
  - Redefine `.glass` and `.glass-strong` as flat panels: `bg-card border border-border rounded-md`
  - _Requirements: 1.1, 1.2, 1.3, 1.5, 2.1, 2.2, 2.5, 3.1, 3.3, 5.1, 6.1, 7.2, 8.1, 9.1, 14.1, 14.2_

- [x] 2. Update `tailwind.config.ts`
  - Add `'6': 'hsl(var(--chart-6))'` to the `chart` color map
  - Add `fontFamily` extension: `sans: ['var(--font-source-sans)', 'system-ui', 'sans-serif']` and `mono: ['var(--font-jetbrains-mono)', 'monospace']`
  - _Requirements: 3.1, 4.1, 4.2, 14.1_

- [x] 3. Update `app/layout.tsx` — replace font imports
  - Remove `Inter` import; add `Source_Sans_3` and `JetBrains_Mono` imports from `next/font/google`
  - Configure `Source_Sans_3` with `variable: '--font-source-sans'` and `display: 'swap'`
  - Configure `JetBrains_Mono` with `variable: '--font-jetbrains-mono'` and `display: 'swap'`
  - Apply both `.variable` classes to `<body>` alongside `font-sans antialiased bg-background text-foreground`
  - _Requirements: 4.1, 4.2, 4.5_

- [x] 4. Update `components/Sidebar.tsx` — remove glassmorphism, replace emerald tokens
  - Replace workspace icon container class `bg-gradient-to-br from-emerald-400 to-emerald-600` with `bg-primary/15`
  - Replace workspace icon color `text-white` with `text-primary-foreground`
  - Replace "Workspace" label `text-emerald-400` with `text-primary`
  - Replace active nav item classes (`bg-emerald-500/10 text-emerald-400 border-emerald-500/20`) with `border-l-2 border-primary bg-primary/10 text-primary font-medium`
  - Replace inactive nav item classes (`text-neutral-400 hover:bg-neutral-800/50`) with `border-l-2 border-transparent text-muted-foreground hover:bg-muted hover:text-foreground`
  - Replace workspace dropdown container classes (`bg-neutral-900 border-neutral-800`) with `bg-card border-border`
  - Replace active workspace in dropdown (`bg-emerald-500/10 text-emerald-400`) with `bg-primary/10 text-primary`
  - Replace inactive workspace in dropdown (`text-neutral-300 hover:bg-neutral-800`) with `text-foreground hover:bg-muted`
  - Replace footer background `bg-neutral-950/30` with `bg-background`
  - Replace footer text class with `font-mono text-[11px] text-muted-foreground/50`
  - Replace logout hover classes (`hover:text-red-400 hover:bg-red-500/10`) with `hover:text-destructive hover:bg-destructive/10`
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 14.3, 14.4_

- [x] 5. Update `components/DashboardLayout.tsx` — remove decorative glows
  - Remove the two absolute-positioned glow `<div>` elements (`bg-primary/10 blur-[120px]` and `bg-blue-500/10 blur-[120px]`)
  - Remove `relative` from the outer wrapper class
  - _Requirements: 7.2, 7.5, 14.4_

- [x] 6. Update `components/MetricCard.tsx` — monospace value, flat icon container
  - Add `font-mono` class to the `<h3>` value element
  - Replace icon container class `bg-white/5 backdrop-blur-sm` with `bg-primary/15`
  - Replace trend color `text-emerald-400` with `text-primary` for the downward trend
  - _Requirements: 4.3, 8.3, 8.4, 8.5, 14.3, 14.4_

- [x] 7. Create `web/frontend/lib/chart-colors.ts` — `chartColor()` utility
  - Implement `chartColor(n: 1 | 2 | 3 | 4 | 5 | 6): string` that reads `--chart-N` via `getComputedStyle` on the client and wraps the HSL channels in `hsl()`
  - Add SSR fallback: when `typeof window === 'undefined'`, return the hardcoded W&B defaults (chart-1 through chart-6 as specified in the design)
  - _Requirements: 3.1, 3.3, 14.1_

- [x] 8. Update chart components — use `chartColor()`, update Recharts props
  - [x] 8.1 Update `components/EnergyChart.tsx`
    - Import `chartColor` from `lib/chart-colors`
    - Replace static `GPU_COLORS` array with `[1,2,3,4,5,6].map(n => chartColor(n as 1|2|3|4|5|6))`
    - Replace hardcoded CPU/memory/iGPU stroke colors with `chartColor(1)`, `chartColor(3)`, `chartColor(4)`
    - Update `<CartesianGrid>` stroke to resolve `--border` at render time with 0.6 opacity
    - Update `<XAxis>` and `<YAxis>` stroke and tick fill to use `hsl(var(--border))` and `hsl(var(--muted-foreground))`
    - Update `<ReferenceLine>` stroke to `hsl(var(--muted-foreground))`
    - Update tooltip `contentStyle` to use card/border/foreground tokens
    - _Requirements: 3.1, 3.3, 9.3, 9.4, 9.5, 14.3, 14.4_
  - [x] 8.2 Update `components/CumulativePowerChart.tsx`
    - Same Recharts prop updates as 8.1 (CartesianGrid, XAxis, YAxis, tooltip, ReferenceLine)
    - Replace hardcoded series stroke colors with `chartColor()` calls
    - _Requirements: 3.1, 3.3, 9.3, 9.4, 9.5, 14.3, 14.4_
  - [x] 8.3 Update `components/EnergyBreakdownChart.tsx`
    - Replace static `GPU_COLORS` array with `chartColor()` calls
    - Apply Recharts prop updates (CartesianGrid, XAxis, YAxis, tooltip)
    - _Requirements: 3.1, 3.3, 9.3, 9.4, 9.5, 14.3, 14.4_
  - [x] 8.4 Update `components/PowerDistributionChart.tsx`
    - Replace hardcoded `color` values in the `data` array with `chartColor(1)` through `chartColor(4)`
    - Update tooltip `contentStyle` to use card/border/foreground tokens
    - _Requirements: 3.1, 3.3, 9.5, 14.3, 14.4_
  - [x] 8.5 Update `components/UtilizationChart.tsx`
    - Replace hardcoded series stroke colors with `chartColor()` calls
    - Apply Recharts prop updates (CartesianGrid, XAxis, YAxis, tooltip, ReferenceLine)
    - _Requirements: 3.1, 3.3, 9.3, 9.4, 9.5, 14.3, 14.4_
  - [x] 8.6 Update `components/SocketEnergyChart.tsx`
    - Replace `SOCKET_COLORS` object with `chartColor()` calls: `cpu: chartColor(1)`, `dram: chartColor(3)`, `igpu: chartColor(4)`
    - Update `<Brush>` stroke and fill to use `hsl(var(--border))` and `hsl(var(--card))`
    - Apply Recharts prop updates (CartesianGrid, XAxis, YAxis, tooltip)
    - _Requirements: 3.1, 3.3, 9.3, 9.4, 9.5, 14.3, 14.4_

- [x] 9. Checkpoint — verify visual baseline
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Update table components — uppercase headers, monospace numerics, token-based badges
  - [x] 10.1 Update `components/ProjectsTable.tsx`
    - Add `uppercase tracking-widest text-[11px]` to all `<th>` elements
    - Add `font-mono` to numeric cells (energy, run count, duration)
    - _Requirements: 5.3, 10.3, 10.5, 14.3, 14.4_
  - [x] 10.2 Update `components/RunsTable.tsx`
    - Add `uppercase tracking-widest text-[11px]` to all `<th>` elements
    - Add `font-mono` to numeric cells (energy, duration, CO₂)
    - Replace `text-emerald-400` on the finished status icon with `text-primary`
    - _Requirements: 5.3, 10.3, 10.5, 14.3, 14.4_
  - [x] 10.3 Update `components/RecentRunsTable.tsx`
    - Add `uppercase tracking-widest text-[11px]` to all `<th>` elements
    - Add `font-mono` to numeric cells
    - Replace status badge classes: `bg-emerald-500/20 text-emerald-300` → `bg-primary/15 text-primary`; running state → `bg-chart-1/20 text-chart-1`; error state → `bg-destructive/15 text-destructive`
    - _Requirements: 5.3, 10.3, 10.5, 14.3, 14.4_

- [x] 11. Update `app/login/page.tsx` — flat card, gold button, token-based inputs
  - Replace page background `bg-neutral-950` with `bg-background`
  - Remove all decorative glow `<div>` elements (`bg-emerald-500/20 blur-[120px]` etc.)
  - Replace form card classes (`bg-neutral-900/60 backdrop-blur-xl border-neutral-800 rounded-3xl`) with `bg-card border border-border rounded-md`
  - Replace logo icon container (`bg-gradient-to-br from-emerald-400 to-emerald-600 shadow-emerald-500/30`) with `bg-primary/15`
  - Replace logo icon color `text-white` with `text-primary`
  - Replace input field classes: `bg-neutral-950/50 border-neutral-800 focus:ring-emerald-500/50 focus:border-emerald-500` → `bg-background border-border focus:ring-primary/50 focus:border-primary`
  - Replace submit button (`bg-gradient-to-r from-emerald-500 to-emerald-600 shadow-emerald-500/25`) with `bg-primary text-primary-foreground hover:brightness-110`
  - Replace link color `text-emerald-400 hover:text-emerald-300` with `text-primary hover:text-primary/80`
  - Replace error box classes with `bg-destructive/10 border-destructive/20 text-destructive`
  - Replace label `text-neutral-300` with `text-foreground`; placeholder `placeholder:text-neutral-600` with `placeholder:text-muted-foreground/50`
  - _Requirements: 12.1, 12.2, 12.3, 12.5, 14.3, 14.4_

- [x] 12. Update `app/register/page.tsx` — same treatment as login
  - Apply identical changes as task 11 (flat card, gold button, token-based inputs)
  - Replace logo icon container (`bg-gradient-to-br from-blue-400 to-blue-600 shadow-blue-500/30`) with `bg-primary/15`
  - Replace submit button (`bg-gradient-to-r from-blue-500 to-blue-600 shadow-blue-500/25`) with `bg-primary text-primary-foreground hover:brightness-110`
  - Replace link color `text-blue-400 hover:text-blue-300` with `text-primary hover:text-primary/80`
  - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5, 14.3, 14.4_

- [x] 13. Write tests
  - [x] 13.1 Create `web/frontend/__tests__/tokens.test.ts` — token smoke tests
    - Read and parse `globals.css` to extract HSL channel values for each token
    - Assert `--background` lightness is in `[15%, 20%]`
    - Assert `--card` lightness is `--background` lightness + `[3%, 5%]`
    - Assert `--border` lightness is in `[22%, 28%]`
    - Assert `--primary` hue in `[43°, 47°]`, saturation `[95%, 100%]`, lightness `[55%, 62%]`
    - Assert `--chart-1` through `--chart-6` all exist with saturation `[60%, 90%]` and lightness `[60%, 75%]`
    - Assert `--radius` equals `0.375rem`
    - _Requirements: 1.1, 1.2, 1.3, 2.1, 3.1, 3.3, 14.1_
  - [ ]* 13.2 Create `web/frontend/__tests__/contrast.test.ts` — WCAG contrast ratio tests
    - Implement `relativeLuminance` and `contrastRatio` utilities per the design document
    - Assert `--foreground` vs `--background` ≥ 4.5:1
    - Assert `--primary` vs `--primary-foreground` ≥ 4.5:1
    - Assert `--primary` vs `--background` ≥ 3:1
    - Assert `--card` vs `--background` ≥ 1.5:1
    - Assert each `--chart-N` vs `--background` ≥ 3:1
    - _Requirements: 1.4, 2.3, 2.4, 3.2, 5.5, 13.1, 13.2, 13.3_
  - [ ]* 13.3 Create component rendering tests in `web/frontend/__tests__/components/`
    - `MetricCard.test.tsx`: assert value `<h3>` has `font-mono`; icon container has `bg-primary/15` and no `backdrop-blur`
    - `Sidebar.test.tsx`: assert active nav item has `border-l-2 border-primary`; root `<aside>` has no `backdrop-blur`
    - `DashboardLayout.test.tsx`: assert no child element has `blur-[` in className
    - `LoginPage.test.tsx`: assert form card has `bg-card` and `border-border`; no `backdrop-blur`
    - `tables.test.tsx`: assert `<th>` elements have `uppercase` and `tracking-widest`; numeric `<td>` elements have `font-mono`
    - _Requirements: 4.3, 6.1, 6.2, 7.2, 8.1, 8.5, 10.3, 10.5, 12.2_
  - [ ]* 13.4 Create `web/frontend/__tests__/static-analysis.test.ts` — glassmorphism removal
    - Read all component files and assert absence of `backdrop-blur`, `bg-black/`, `blur-[` patterns
    - Assert absence of hardcoded emerald classes: `text-emerald-`, `bg-emerald-`, `border-emerald-`
    - Assert absence of hardcoded neutral classes that should be tokens: `bg-neutral-9`, `text-neutral-`
    - _Requirements: 1.5, 14.3, 14.4_

- [x] 14. Final checkpoint — ensure all tests pass
  - Run `pnpm vitest --run` from `web/frontend` and confirm all tests pass.
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Tasks 1–3 establish the token foundation; all subsequent tasks depend on them being complete first
- Task 7 (`chartColor()`) must be complete before tasks 8.1–8.6
- The design document has no Correctness Properties section — property-based tests are not applicable; only example-based unit tests and static analysis are used
- Run tests with: `pnpm vitest --run` (single execution, no watch mode)
- Each task references specific requirements for traceability
