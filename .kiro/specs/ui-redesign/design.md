# Design Document: UI Redesign (W&B-Inspired)

## Overview

This document describes the technical design for the EcoCode frontend visual redesign. The goal is to replace the current dark glassmorphism aesthetic (near-black background, emerald green accent, `backdrop-blur` everywhere) with a W&B-inspired design language: neutral charcoal dark background, vivid gold/amber primary accent, flat panels with thin 1px borders, and clean humanist sans-serif typography.

The redesign is purely visual — no routing, data fetching, or component structure changes. Every change is either a CSS custom property value, a Tailwind utility class swap, a font import, or a Recharts prop update.

### Key Design Decisions

1. **Single source of truth for tokens**: All color and font tokens live in `globals.css` under `:root`. Components reference Tailwind utilities (`bg-card`, `text-foreground`, `font-mono`) that map to those tokens via `tailwind.config.ts`. No component hardcodes an HSL value.

2. **Flat panels replace glassmorphism**: Every `backdrop-blur`, `bg-black/XX`, and decorative `blur-[Xpx]` glow is removed. Surfaces use solid `bg-card` with a `border border-border` 1px line.

3. **Gold replaces emerald**: `--primary` changes from `hsl(158 94% 45%)` (emerald) to `hsl(45 100% 58%)` (gold). All active states, focus rings, and primary buttons follow.

4. **Source Sans 3 + JetBrains Mono**: Loaded via `next/font/google`, replacing Inter and the Arial fallback. Monospace is applied to all numeric values, API keys, and code content.

5. **Chart colors updated to W&B palette**: All hardcoded `hsl(...)` strings in chart components are replaced with CSS variable references (`var(--chart-1)` etc.) so the token system controls chart colors.

6. **Radius reduced**: `--radius` drops from `0.5rem` to `0.375rem` (6px) to match the W&B flat panel aesthetic.

---

## Architecture

The change set is layered as follows:

```
globals.css          ← token definitions (:root)
tailwind.config.ts   ← wires CSS vars to Tailwind color names + adds chart-6
layout.tsx           ← font loading (Source Sans 3, JetBrains Mono)
globals.css          ← component layer (.metric-card, .chart-container, .sidebar-glass, etc.)
components/          ← class swaps and Recharts prop updates
```

No new dependencies are introduced. `next/font/google` is already available (Next.js 13+). Recharts 2.x is already installed.

### Dependency on Tailwind CSS variable convention

The project uses the shadcn/ui convention: Tailwind color tokens are defined as `hsl(var(--token))` in `tailwind.config.ts`, and the CSS custom properties store only the HSL channel values (e.g., `--background: 0 0% 15%`). This convention is preserved exactly — only the channel values change.

---

## Components and Interfaces

### `globals.css` — Token Layer

All tokens are defined under `:root` in the `@layer base` block. The component utility classes (`.metric-card`, `.chart-container`, `.sidebar-glass`, `.glass`, `.glass-strong`) are redefined in `@layer components` to use flat-panel styles.

**Token changes summary:**

| Token | Current value | New value |
|---|---|---|
| `--background` | `0 0% 3%` | `0 0% 15%` |
| `--foreground` | `0 0% 98%` | `0 0% 96%` |
| `--card` | `0 0% 6%` | `0 0% 18%` |
| `--card-foreground` | `0 0% 98%` | `0 0% 96%` |
| `--popover` | `0 0% 6%` | `0 0% 18%` |
| `--popover-foreground` | `0 0% 98%` | `0 0% 96%` |
| `--primary` | `158 94% 45%` | `45 100% 58%` |
| `--primary-foreground` | `0 0% 0%` | `0 0% 10%` |
| `--secondary` | `0 0% 10%` | `25 80% 65%` |
| `--secondary-foreground` | `0 0% 98%` | `0 0% 10%` |
| `--muted` | `0 0% 15%` | `0 0% 22%` |
| `--muted-foreground` | `0 0% 60%` | `0 0% 63%` |
| `--accent` | `158 94% 45%` | `45 100% 58%` |
| `--accent-foreground` | `0 0% 0%` | `0 0% 10%` |
| `--destructive` | `0 84.2% 60.2%` | `2 78% 60%` |
| `--destructive-foreground` | `0 0% 98%` | `0 0% 96%` |
| `--border` | `0 0% 12%` | `0 0% 24%` |
| `--input` | `0 0% 12%` | `0 0% 24%` |
| `--ring` | `158 94% 45%` | `45 100% 58%` |
| `--chart-1` | `158 94% 45%` | `199 90% 64%` |
| `--chart-2` | `199 89% 48%` | `14 100% 70%` |
| `--chart-3` | `280 65% 60%` | `123 35% 64%` |
| `--chart-4` | `49 89% 52%` | `291 45% 71%` |
| `--chart-5` | `340 75% 55%` | `0 70% 76%` |
| `--chart-6` | *(absent)* | `54 100% 73%` |
| `--radius` | `0.5rem` | `0.375rem` |
| `--font-sans` | *(absent)* | `var(--font-source-sans)` |
| `--font-mono` | *(absent)* | `var(--font-jetbrains-mono)` |

**Component utility class redefinitions:**

```css
/* BEFORE */
.metric-card {
  @apply bg-black/20 backdrop-blur-lg p-6 rounded-xl border border-white/10
         transition-all duration-300 hover:border-primary/50 hover:bg-black/40;
}

/* AFTER */
.metric-card {
  @apply bg-card p-6 rounded-md border border-border
         transition-all duration-300 hover:border-primary/40;
}
```

```css
/* BEFORE */
.chart-container {
  @apply bg-black/20 backdrop-blur-lg p-6 rounded-xl border border-white/10
         transition-all duration-300 hover:border-white/20;
}

/* AFTER */
.chart-container {
  @apply bg-card p-6 rounded-md border border-border
         transition-all duration-300;
}
```

```css
/* BEFORE */
.sidebar-glass {
  @apply bg-black/60 backdrop-blur-2xl border-r border-white/10;
}

/* AFTER */
.sidebar-glass {
  @apply bg-card border-r border-border;
}
```

The `.glass` and `.glass-strong` classes are kept in the file but redefined as flat panels (they may be used by shadcn/ui components):

```css
.glass {
  @apply bg-card border border-border rounded-md;
}
.glass-strong {
  @apply bg-card border border-border rounded-md;
}
```

### `tailwind.config.ts` — Token Wiring

One addition: `chart-6` must be added to the `chart` color map so `bg-chart-6` and `stroke-chart-6` utilities are available:

```ts
chart: {
  '1': 'hsl(var(--chart-1))',
  '2': 'hsl(var(--chart-2))',
  '3': 'hsl(var(--chart-3))',
  '4': 'hsl(var(--chart-4))',
  '5': 'hsl(var(--chart-5))',
  '6': 'hsl(var(--chart-6))',   // ← add this
},
```

No other changes to `tailwind.config.ts`.

### `layout.tsx` — Font Loading

Replace the `Inter` import with `Source_Sans_3` and `JetBrains_Mono`:

```tsx
import { Source_Sans_3, JetBrains_Mono } from 'next/font/google'

const sourceSans = Source_Sans_3({
  subsets: ['latin'],
  variable: '--font-source-sans',
  display: 'swap',
})

const jetbrainsMono = JetBrains_Mono({
  subsets: ['latin'],
  variable: '--font-jetbrains-mono',
  display: 'swap',
})
```

Apply both CSS variable classes to `<body>`:

```tsx
<body className={`${sourceSans.variable} ${jetbrainsMono.variable} font-sans antialiased bg-background text-foreground`}>
```

The `font-sans` Tailwind utility must be wired to `--font-source-sans` in `tailwind.config.ts`:

```ts
fontFamily: {
  sans: ['var(--font-source-sans)', 'system-ui', 'sans-serif'],
  mono: ['var(--font-jetbrains-mono)', 'monospace'],
},
```

### `Sidebar.tsx` — Visual Changes

| Element | Current | New |
|---|---|---|
| Root `<aside>` | `sidebar-glass` (glassmorphism) | `sidebar-glass` (now flat via redefined class) |
| Workspace icon container | `bg-gradient-to-br from-emerald-400 to-emerald-600` | `bg-primary/15` |
| Workspace icon | `text-white` | `text-primary-foreground` |
| "Workspace" label | `text-emerald-400` | `text-primary` |
| Active nav item | `bg-emerald-500/10 text-emerald-400 border-emerald-500/20` | `border-l-2 border-primary bg-primary/10 text-primary` |
| Inactive nav item | `text-neutral-400 hover:bg-neutral-800/50` | `text-muted-foreground hover:bg-muted` |
| Workspace dropdown bg | `bg-neutral-900 border-neutral-800` | `bg-card border-border` |
| Active workspace in dropdown | `bg-emerald-500/10 text-emerald-400` | `bg-primary/10 text-primary` |
| Inactive workspace in dropdown | `text-neutral-300 hover:bg-neutral-800` | `text-foreground hover:bg-muted` |
| Footer | `bg-neutral-950/30` | `bg-background` |
| Footer text | `text-muted-foreground/60` | `font-mono text-[11px] text-muted-foreground/50` |
| Logout button hover | `hover:text-red-400 hover:bg-red-500/10` | `hover:text-destructive hover:bg-destructive/10` |

The active nav item gets a left border accent instead of a full border box:

```tsx
// Active
className="flex items-center gap-3 px-3 py-2.5 rounded-md transition-all duration-200
           border-l-2 border-primary bg-primary/10 text-primary font-medium"

// Inactive
className="flex items-center gap-3 px-3 py-2.5 rounded-md transition-all duration-200
           border-l-2 border-transparent text-muted-foreground hover:bg-muted hover:text-foreground"
```

### `DashboardLayout.tsx` — Remove Glows

Remove the two decorative `<div>` glow elements entirely. The layout becomes:

```tsx
<div className="flex flex-col lg:flex-row h-screen bg-background overflow-hidden">
  <Sidebar />
  <main className="flex-1 overflow-auto">
    <div className="min-h-screen">{children}</div>
  </main>
</div>
```

No `relative`, no `absolute` glow divs, no `blur-[120px]`.

### `MetricCard.tsx` — Visual Changes

The `.metric-card` CSS class handles the background/border change. Inside the component:

| Element | Current | New |
|---|---|---|
| Value (`h3`) | `text-foreground` (implicit) | Add `font-mono` class |
| Icon container | `bg-white/5 backdrop-blur-sm` | `bg-primary/15` |
| Icon | `text-primary` | `text-primary` (unchanged) |

```tsx
// Value element
<h3 className="text-2xl md:text-3xl font-bold font-mono text-foreground">
  {value}
</h3>

// Icon container
<div className="ml-4 p-2 rounded bg-primary/15">
  <Icon className="w-5 h-5 text-primary" />
</div>
```

### Chart Components — Recharts Prop Updates

All chart components currently hardcode HSL strings directly in Recharts props (`stroke="hsl(158 94% 45%)"`, `fill="hsl(199 89% 48%)"`, etc.). These must be replaced with CSS variable references.

**The mapping from semantic role to new token:**

| Semantic role | Old hardcoded HSL | New CSS variable |
|---|---|---|
| CPU | `hsl(158 94% 45%)` | `var(--chart-1)` → cyan `hsl(199 90% 64%)` |
| GPU (total / GPU 0) | `hsl(199 89% 48%)` | `var(--chart-2)` → orange `hsl(14 100% 70%)` |
| Memory | `hsl(20 89% 55%)` | `var(--chart-3)` → green `hsl(123 35% 64%)` |
| iGPU | `hsl(49 89% 52%)` | `var(--chart-4)` → purple `hsl(291 45% 71%)` |
| GPU 1 | `hsl(280 65% 60%)` | `var(--chart-4)` → purple |
| GPU 2 | `hsl(49 89% 52%)` | `var(--chart-6)` → yellow |
| GPU 3+ | remaining GPU_COLORS | cycle through `--chart-1` … `--chart-6` |

**Recharts does not read CSS variables from `stroke`/`fill` props directly** when those props are SVG attributes — SVG `stroke` and `fill` attributes do not resolve CSS custom properties in all browsers. The correct approach is to resolve the variable at render time using a helper:

```ts
// utils/chart-colors.ts
export function chartColor(n: 1 | 2 | 3 | 4 | 5 | 6): string {
  if (typeof window === 'undefined') {
    // SSR fallback — return the W&B defaults directly
    const defaults: Record<number, string> = {
      1: 'hsl(199 90% 64%)',
      2: 'hsl(14 100% 70%)',
      3: 'hsl(123 35% 64%)',
      4: 'hsl(291 45% 71%)',
      5: 'hsl(0 70% 76%)',
      6: 'hsl(54 100% 73%)',
    }
    return defaults[n]
  }
  return getComputedStyle(document.documentElement)
    .getPropertyValue(`--chart-${n}`)
    .trim()
    // CSS var stores "H S% L%" — wrap in hsl()
    .replace(/^(.+)$/, 'hsl($1)')
}
```

Each chart component calls `chartColor(n)` for its series colors. Because chart components are already `'use client'`, `window` is available at render time.

**Tooltip `contentStyle` updates** (same in all charts):

```tsx
contentStyle={{
  backgroundColor: 'hsl(var(--card))',
  border: '1px solid hsl(var(--border))',
  borderRadius: '4px',
  fontSize: '12px',
  color: 'hsl(var(--foreground))',
}}
```

**CartesianGrid stroke** (all charts):

```tsx
<CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border) / 0.6)" />
```

Note: CSS `hsl()` with `/` opacity syntax works in modern browsers. For Recharts SVG attributes, use the resolved value from `chartColor()` or inline `rgba`. A safe alternative:

```tsx
// Resolve border color at render time
const borderColor = typeof window !== 'undefined'
  ? getComputedStyle(document.documentElement).getPropertyValue('--border').trim()
  : '0 0% 24%'
const gridStroke = `hsla(${borderColor}, 0.6)`
```

**XAxis / YAxis tick and stroke** (all charts):

```tsx
stroke="hsl(var(--border))"
tick={{ fontSize: 10, fill: 'hsl(var(--muted-foreground))' }}
```

**ReferenceLine separator** (EnergyChart, CumulativePowerChart, UtilizationChart):

```tsx
stroke="hsl(var(--muted-foreground))"
```

**SeparatorLabel text fill** (all charts with run separators):

```tsx
fill="hsl(var(--muted-foreground))"
```

**SocketEnergyChart `Brush`**:

```tsx
<Brush
  dataKey="time"
  height={30}
  stroke="hsl(var(--border))"
  fill="hsl(var(--card))"
  travellerWidth={10}
/>
```

**GPU_COLORS arrays** in EnergyChart, EnergyBreakdownChart, UtilizationChart — replace with a function call:

```ts
// Replace the static GPU_COLORS array
const GPU_COLORS = [1, 2, 3, 4, 5, 6].map(n => chartColor(n as 1|2|3|4|5|6))
```

**SocketEnergyChart `SOCKET_COLORS`**:

```ts
const SOCKET_COLORS: Record<string, string> = {
  cpu:  chartColor(1),   // cyan
  dram: chartColor(3),   // green
  igpu: chartColor(4),   // purple
}
```

**PowerDistributionChart `data` array**:

```tsx
const data = [
  { name: 'CPU',    value: ..., color: chartColor(1) },
  { name: 'GPU',    value: ..., color: chartColor(2) },
  { name: 'Memory', value: ..., color: chartColor(3) },
  { name: 'iGPU',   value: ..., color: chartColor(4) },
].filter(item => item.value > 0)
```

### Table Components — Visual Changes

The `.chart-container` class already handles the background/border change. Inside each table:

**Column headers** — add `uppercase tracking-widest text-[11px]` to `<th>` elements:

```tsx
<th className="text-left py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px]">
  Project Name
</th>
```

**Numeric cells** — add `font-mono` to cells containing energy, duration, CO₂, and count values:

```tsx
<td className="text-right py-3 px-4 text-foreground font-medium font-mono">
  {Number(run.totalEnergy.toFixed(2)).toLocaleString()}
</td>
```

**Row hover** — already uses `hover:bg-muted/30` which is correct with the new muted token.

**RecentRunsTable status badge** — replace emerald/blue hardcoded colors with token-based classes:

```tsx
// BEFORE
className={`... ${run.status === 'finished' ? 'bg-emerald-500/20 text-emerald-300' : ...}`}

// AFTER
className={`inline-block px-3 py-1 rounded text-xs font-medium ${
  run.status === 'finished' ? 'bg-primary/15 text-primary'
  : run.status === 'running' ? 'bg-chart-1/20 text-chart-1'
  : 'bg-destructive/15 text-destructive'
}`}
```

**RunsTable status icon** — replace `text-emerald-400` with `text-primary`:

```tsx
case 'finished':
  return <Check className="w-4 h-4 text-primary" />
```

### Login and Register Pages — Visual Changes

Both pages share the same treatment:

| Element | Current | New |
|---|---|---|
| Page background | `bg-neutral-950` | `bg-background` |
| Background glow divs | `bg-emerald-500/20 blur-[120px]` etc. | Remove entirely |
| Form card | `bg-neutral-900/60 backdrop-blur-xl border-neutral-800 rounded-3xl` | `bg-card border border-border rounded-md` |
| Logo icon container (login) | `bg-gradient-to-br from-emerald-400 to-emerald-600 shadow-emerald-500/30` | `bg-primary/15` |
| Logo icon (login) | `text-white` | `text-primary` |
| Logo icon container (register) | `bg-gradient-to-br from-blue-400 to-blue-600 shadow-blue-500/30` | `bg-primary/15` |
| Logo icon (register) | `text-white` | `text-primary` |
| Input fields | `bg-neutral-950/50 border-neutral-800 focus:ring-emerald-500/50 focus:border-emerald-500` | `bg-background border-border focus:ring-primary/50 focus:border-primary` |
| Submit button (login) | `bg-gradient-to-r from-emerald-500 to-emerald-600 shadow-emerald-500/25` | `bg-primary text-primary-foreground hover:brightness-110` |
| Submit button (register) | `bg-gradient-to-r from-blue-500 to-blue-600 shadow-blue-500/25` | `bg-primary text-primary-foreground hover:brightness-110` |
| Link color (login) | `text-emerald-400 hover:text-emerald-300` | `text-primary hover:text-primary/80` |
| Link color (register) | `text-blue-400 hover:text-blue-300` | `text-primary hover:text-primary/80` |
| Error box | `bg-red-500/10 border-red-500/20 text-red-400` | `bg-destructive/10 border-destructive/20 text-destructive` |
| Label text | `text-neutral-300` | `text-foreground` |
| Placeholder text | `placeholder:text-neutral-600` | `placeholder:text-muted-foreground/50` |

---

## Data Models

No data model changes. This is a purely visual redesign.

The only new "data" introduced is the `chartColor()` utility function, which reads CSS custom properties at runtime. It has no persistent state.

---

## Correctness Properties

PBT is not applicable to this feature.

This redesign consists entirely of CSS custom property value changes, Tailwind utility class swaps, font configuration, and Recharts prop updates. There are no pure functions with meaningful input variation, no parsers or serializers, and no business logic. All requirements are either static configuration checks (verifying token values in `globals.css`) or specific rendering checks (verifying that a component applies the correct CSS class to a specific element).

Running 100 iterations of any test here would not find more bugs than running it once — the inputs are fixed token values and fixed component structures. Property-based testing is therefore not appropriate.

The appropriate testing strategies are:

- **Smoke tests**: Parse `globals.css` to verify token values are in the specified ranges (lightness, hue, saturation).
- **Example-based unit tests**: Compute WCAG contrast ratios between specific token pairs and assert they meet AA thresholds.
- **Component rendering tests**: Render components and assert specific CSS classes are present on specific elements.
- **Static analysis**: Grep component files to verify absence of hardcoded color values and glassmorphism classes.

---

## Error Handling

### Font Loading Failures

`next/font/google` fetches fonts at build time and bundles them. If the Google Fonts CDN is unreachable at build time, the build fails with a clear error. The `display: 'swap'` option ensures the fallback system font renders immediately while the custom font loads, preventing invisible text (FOIT).

Fallback font stacks:
- Sans: `var(--font-source-sans), system-ui, -apple-system, sans-serif`
- Mono: `var(--font-jetbrains-mono), 'Courier New', monospace`

### CSS Variable Resolution in Recharts

The `chartColor()` helper handles the SSR case by returning hardcoded HSL fallbacks when `window` is undefined. On the client, it reads the resolved CSS variable. If a variable is missing (e.g., `--chart-6` not yet added to `tailwind.config.ts`), `getPropertyValue` returns an empty string, and the `hsl()` wrapper produces `hsl()` which renders as black. This is a visible failure that will be caught immediately in development.

### Token Misconfiguration

If a token value is set outside the specified range (e.g., `--primary` lightness too low), the contrast ratio tests will catch it. The WCAG contrast ratio computation is deterministic and will fail the test with a clear numeric output.

---

## Testing Strategy

This feature does not use property-based testing. The appropriate test types are:

### 1. Token Value Smoke Tests

Parse `globals.css` and verify each token's HSL channels are within the specified ranges. These run once and are fast.

**Examples:**
- `--background` lightness is in `[15%, 20%]`
- `--card` lightness is `--background` lightness + `[3%, 5%]`
- `--border` lightness is in `[22%, 28%]`
- `--primary` hue is in `[43°, 47°]`, saturation `[95%, 100%]`, lightness `[55%, 62%]`
- `--chart-1` through `--chart-6` all exist and have saturation `[60%, 90%]`, lightness `[60%, 75%]`
- `--radius` is `0.375rem`

### 2. WCAG Contrast Ratio Tests

Implement a `wcagContrast(hsl1, hsl2)` function and assert ratios for all specified pairs. These are example-based unit tests.

**Pairs to test:**
- `--foreground` vs `--background` → ≥ 4.5:1 (body text, Req 5.5, 13.1)
- `--primary` vs `--primary-foreground` → ≥ 4.5:1 (button text, Req 2.3, 11.1)
- `--primary` vs `--background` → ≥ 3:1 (gold as UI component color, Req 2.4, 13.3)
- `--card` vs `--background` → ≥ 1.5:1 (surface differentiation, Req 1.4)
- Each `--chart-N` vs `--background` → ≥ 3:1 (chart readability, Req 3.2)

A suitable test framework is **Vitest** (already compatible with the Next.js/TypeScript stack). The `wcagContrast` function is a pure utility with no dependencies.

```ts
// lib/wcag.ts
export function relativeLuminance(r: number, g: number, b: number): number {
  const [rs, gs, bs] = [r, g, b].map(c => {
    const s = c / 255
    return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4)
  })
  return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs
}

export function contrastRatio(l1: number, l2: number): number {
  const [lighter, darker] = l1 > l2 ? [l1, l2] : [l2, l1]
  return (lighter + 0.05) / (darker + 0.05)
}
```

### 3. Component Rendering Tests

Use **React Testing Library** + Vitest to render components and assert class presence.

**Key assertions:**
- `MetricCard` value element has `font-mono` class
- `MetricCard` icon container has `bg-primary/15` class (no `backdrop-blur`)
- `Sidebar` active nav item has `border-l-2 border-primary` classes
- `Sidebar` root element has no `backdrop-blur` class
- `DashboardLayout` has no child elements with `blur-[` in their className
- Login page form card has `bg-card` and `border-border` classes (no `backdrop-blur`)
- Table `<th>` elements have `uppercase` and `tracking-widest` classes
- Table numeric `<td>` elements have `font-mono` class

### 4. Static Analysis (Glassmorphism Removal)

A simple grep-based test (or a custom ESLint rule) verifies that no component file contains:
- `backdrop-blur`
- `bg-black/`
- `blur-[` (decorative glow pattern)
- Hardcoded emerald color classes: `text-emerald-`, `bg-emerald-`, `border-emerald-`
- Hardcoded neutral classes that should be tokens: `bg-neutral-9`, `text-neutral-`

This can be implemented as a Vitest test that reads the file system and runs regex assertions.

### Test File Organization

```
web/frontend/
  __tests__/
    tokens.test.ts          ← smoke tests for globals.css token values
    contrast.test.ts        ← WCAG contrast ratio example tests
    components/
      MetricCard.test.tsx   ← component rendering tests
      Sidebar.test.tsx
      DashboardLayout.test.tsx
      LoginPage.test.tsx
      tables.test.tsx
    static-analysis.test.ts ← glassmorphism removal grep tests
```

Run with: `pnpm vitest --run` (single execution, no watch mode).
