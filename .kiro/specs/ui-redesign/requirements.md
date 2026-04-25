# Requirements Document

## Introduction

EcoCode is a Next.js web application that monitors energy consumption and carbon emissions for ML and compute workloads. The current UI uses a dark glassmorphism aesthetic with emerald green as the primary accent color. This feature redesigns the visual design, color palette, and typography to produce an interface inspired by the Weights & Biases (W&B) web application — a high-information-density developer tool with a neutral charcoal dark background, a vivid gold/amber primary accent, flat panels with subtle borders, and clean humanist sans-serif typography. The redesign covers the sidebar, dashboard, charts, tables, settings pages, login/register pages, and all shared UI components. The existing component structure and routing are preserved; only visual presentation changes.

## Glossary

- **Design_System**: The collection of CSS custom properties, Tailwind configuration, and shared component styles that define the visual language of the application.
- **Color_Palette**: The set of named color tokens (background, foreground, primary, secondary, muted, accent, border, chart colors) defined as CSS custom properties in `globals.css`.
- **Typography_System**: The combination of font families, size scale, weight scale, and line-height values applied across the application.
- **Sidebar**: The fixed left-hand navigation component (`components/Sidebar.tsx`) containing workspace switcher, nav links, and logout.
- **Dashboard**: The main workspace page (`app/[workspaceSlug]/page.tsx`) containing metric cards, charts, and the projects table.
- **MetricCard**: The reusable card component (`components/MetricCard.tsx`) that displays a single KPI with icon, label, value, and unit.
- **Chart_Container**: The styled wrapper applied to all Recharts-based chart components via the `.chart-container` CSS class.
- **Table**: Any data table component in the application (ProjectsTable, RunsTable, RecentRunsTable).
- **Contrast_Ratio**: The luminance ratio between foreground and background colors as defined by WCAG 2.1.
- **Charcoal_Dark**: The W&B-inspired dark theme variant where the background sits at approximately 15–20% lightness using a hue-neutral or very slightly warm gray (e.g., `hsl(0 0% 15%)` to `hsl(30 5% 18%)`). This is distinctly different from near-black (≤8% lightness) and from blue-tinted dark themes.
- **Gold_Accent**: The W&B signature primary accent color — a vivid warm yellow-gold at approximately `hsl(45 100% 58%)` (`#FFCC33`). High-contrast on dark surfaces, used for primary buttons, active nav indicators, and key interactive elements.
- **Flat_Panel**: A surface element with a solid background color, a thin 1px border, and no blur, shadow, or glassmorphism effects.

## Requirements

### Requirement 1: Color Palette — Base Surfaces

**User Story:** As a developer using EcoCode for extended monitoring sessions, I want a neutral charcoal dark background that is not pure black or blue-tinted, so that the interface feels grounded and professional during long sessions.

#### Acceptance Criteria

1. THE Design_System SHALL define a background color token as a hue-neutral or very slightly warm dark gray at 15–20% lightness (e.g., `hsl(0 0% 15%)` or `hsl(30 5% 18%)`), replacing the current near-black `hsl(0 0% 3%)`.
2. THE Design_System SHALL define a card/surface color token that is 3–5% lighter than the background token (lightness in the 18–23% range, e.g., `hsl(0 0% 18%)` or `hsl(30 4% 20%)`), creating a visible but subtle elevation layer with no blur or transparency.
3. THE Design_System SHALL define a border color token at approximately 22–28% lightness (e.g., `hsl(0 0% 24%)`), providing structural separation as a thin 1px line without harsh contrast.
4. WHEN any surface color is rendered against the background, THE Design_System SHALL ensure the resulting contrast ratio is at least 1.5:1 for surface differentiation.
5. THE Design_System SHALL remove all glassmorphism effects (`backdrop-blur`, `bg-black/XX` transparency, `blur-[Xpx]` decorative glows) from surface and layout components, replacing them with Flat_Panel styles.

---

### Requirement 2: Color Palette — Primary and Accent Colors

**User Story:** As a user, I want the primary interactive color to be a vivid gold/amber that stands out clearly on the dark background, so that the interface communicates a strong, recognizable identity aligned with the W&B design language.

#### Acceptance Criteria

1. THE Design_System SHALL define a primary color token as a vivid warm gold (hue 43–47°, saturation 95–100%, lightness 55–62%, e.g., `hsl(45 100% 58%)`), replacing the current emerald green. This token is used for primary buttons, active navigation indicators, focus rings, and key interactive highlights.
2. THE Design_System SHALL define a secondary accent color token as a warm orange or coral (hue 20–30°, saturation 70–85%, lightness 60–70%, e.g., `hsl(25 80% 65%)`), used for secondary highlights and status indicators distinct from the primary gold.
3. WHEN the primary gold token is used as a button background, THE Design_System SHALL pair it with a dark foreground text color (lightness ≤15%) to achieve a contrast ratio of at least 4.5:1 (WCAG 2.1 AA).
4. WHEN the primary gold token is used as text or an icon on a Charcoal_Dark surface, THE Design_System SHALL verify the contrast ratio meets WCAG 2.1 AA (4.5:1 for normal text, 3:1 for large text and UI components).
5. THE Design_System SHALL define a destructive color token as a vivid red (hue 0–5°, saturation 70–85%, lightness 55–65%) for error states and alerts.

---

### Requirement 3: Color Palette — Chart Colors

**User Story:** As a user reading energy charts, I want chart series colors to be vivid and perceptually distinct on the dark background, so that I can differentiate CPU, GPU, memory, and iGPU lines without confusion.

#### Acceptance Criteria

1. THE Design_System SHALL define at least 6 chart color tokens (`--chart-1` through `--chart-6`) as medium-vivid colors (saturation 60–90%, lightness 60–75%) that are perceptually distinct from one another when rendered on the Charcoal_Dark background. These are NOT pastel — they are vivid enough to read clearly on a dark surface.
2. WHEN chart colors are rendered as lines or fills on the Charcoal_Dark background, THE Design_System SHALL ensure each chart color has a contrast ratio of at least 3:1 against the background.
3. THE Design_System SHALL assign chart colors using the following W&B-inspired mapping: `--chart-1` cyan (`#4FC3F7`, approximately `hsl(199 90% 64%)`), `--chart-2` orange (`#FF8A65`, approximately `hsl(14 100% 70%)`), `--chart-3` green (`#81C784`, approximately `hsl(123 35% 64%)`), `--chart-4` purple (`#CE93D8`, approximately `hsl(291 45% 71%)`), `--chart-5` red (`#EF9A9A`, approximately `hsl(0 70% 76%)`), `--chart-6` yellow (`#FFF176`, approximately `hsl(54 100% 73%)`), maintaining consistent semantic mapping across all chart components.
4. THE Design_System SHALL ensure chart colors are not reused for non-chart UI elements (buttons, badges, status indicators) to preserve semantic clarity.

---

### Requirement 4: Typography — Font Family

**User Story:** As a developer, I want the interface to use Source Sans 3 as the primary UI font and a monospace font for numeric and code content, so that the UI feels clean, humanist, and native to a developer environment.

#### Acceptance Criteria

1. THE Typography_System SHALL use Source Sans 3 (loaded via `next/font/google`) as the primary body and UI font, replacing the current Arial/Helvetica fallback. Source Sans 3 is a clean humanist sans-serif designed for UI legibility.
2. THE Typography_System SHALL define a secondary monospace font — JetBrains Mono or IBM Plex Mono (loaded via `next/font/google`) — applied to all numeric metric values, code snippets, API keys, and terminal-style content.
3. WHEN a MetricCard renders a numeric value, THE MetricCard SHALL apply the monospace font class to the value element.
4. WHEN a settings page renders an API key or token, THE Typography_System SHALL apply the monospace font class to that element.
5. THE Typography_System SHALL set a base font size of 14–16px for body text and define a consistent type scale (xs, sm, base, lg, xl, 2xl, 3xl) using `rem` units.

---

### Requirement 5: Typography — Readability and Hierarchy

**User Story:** As a user navigating the dashboard, I want clear visual hierarchy between headings, labels, and body text, so that I can scan the page quickly and find information without effort.

#### Acceptance Criteria

1. THE Typography_System SHALL define at least three distinct text color levels: primary foreground (near-white, approximately `#F5F5F5` / `hsl(0 0% 96%)`), secondary/muted foreground (medium gray, approximately `#A0A0A0` / `hsl(0 0% 63%)`), and disabled/placeholder (dim gray, approximately `#606060` / `hsl(0 0% 38%)`).
2. WHEN a page heading (h1, h2) is rendered, THE Typography_System SHALL apply a font weight of 600–700 and a font size of at least 1.5rem.
3. WHEN a section label or table column header is rendered, THE Typography_System SHALL apply uppercase letter-spacing (`tracking-widest` or equivalent) and the muted foreground color to create visual separation from data content.
4. THE Typography_System SHALL set a line-height of 1.5–1.6 for body text and 1.2–1.3 for headings to ensure comfortable reading density.
5. WHEN body text is rendered against any surface, THE Typography_System SHALL ensure a contrast ratio of at least 4.5:1 (WCAG 2.1 AA) using near-white primary foreground text against the Charcoal_Dark background.

---

### Requirement 6: Sidebar Visual Design

**User Story:** As a user, I want the sidebar to feel structured and unobtrusive in the W&B style, so that it supports navigation without competing with the main content area.

#### Acceptance Criteria

1. THE Sidebar SHALL use the card/surface color token as its background, with a thin 1px right border using the border color token, replacing the current heavy glassmorphism (`bg-black/60 backdrop-blur-2xl`). No blur, no transparency, no shadow.
2. WHEN a navigation item is active, THE Sidebar SHALL highlight it using a 2–3px left border in the Gold_Accent color and a low-opacity gold background tint (approximately 8–12% opacity), replacing the current emerald-colored active state.
3. WHEN a navigation item is hovered, THE Sidebar SHALL apply a subtle background tint using the muted color token without changing the text color to the primary color.
4. THE Sidebar SHALL render navigation item labels using Source Sans 3 at 14px with normal weight (400) for inactive items and medium weight (500) for the active item.
5. THE Sidebar SHALL render the workspace name using Source Sans 3 at 13px with semibold weight (600).
6. THE Sidebar SHALL render the "EcoCode Platform" footer text using the monospace font at 11px in the disabled/placeholder text color.
7. THE Sidebar SHALL remove the emerald gradient from the workspace icon, replacing it with a flat Gold_Accent background or a neutral surface background with a gold icon.

---

### Requirement 7: Dashboard Layout and Spacing

**User Story:** As a user, I want the dashboard to use consistent spacing and a clear grid structure in the W&B high-information-density style, so that I can visually parse sections without cognitive effort.

#### Acceptance Criteria

1. THE Dashboard SHALL use a consistent spacing unit of 8px (0.5rem) as the base, with section gaps of 24px (1.5rem) and card gaps of 16px (1rem).
2. THE Dashboard SHALL remove all decorative background glow elements (`bg-primary/10 blur-[120px]`, `bg-accent/5 blur-[100px]`) from `DashboardLayout`, replacing them with a flat Charcoal_Dark background.
3. WHEN a page section heading (e.g., "Project Overview", "Charts") is rendered, THE Dashboard SHALL apply a thin bottom border using the border color token or a small uppercase tracking label in the muted foreground color to visually anchor the section.
4. THE Dashboard SHALL maintain the existing responsive grid breakpoints (sm, lg, xl) for metric cards and chart layouts.
5. THE Dashboard SHALL use minimal decorative elements — no gradients, no blurs, no rounded corners larger than 6px on panels — consistent with the W&B flat panel aesthetic.

---

### Requirement 8: MetricCard Visual Design

**User Story:** As a user monitoring KPIs, I want metric cards to be clean, flat, and scannable in the W&B style, so that I can read values at a glance without visual noise.

#### Acceptance Criteria

1. THE MetricCard SHALL use the card/surface color token as its background and the border color token for its 1px border, replacing the current `bg-black/20 backdrop-blur-lg` glassmorphism style. No blur, no transparency.
2. WHEN a MetricCard is hovered, THE MetricCard SHALL apply a border color transition to the Gold_Accent token at 40–50% opacity, providing clear interactive feedback.
3. THE MetricCard SHALL render the metric value using the monospace font at 2xl–3xl size and bold weight (700).
4. THE MetricCard SHALL render the metric label using the muted foreground color at 12–13px in Source Sans 3.
5. THE MetricCard SHALL render the icon in a container using the Gold_Accent token at 15–20% opacity as background and the Gold_Accent token for the icon color itself.

---

### Requirement 9: Chart Container Visual Design

**User Story:** As a user reading charts, I want chart containers to integrate cleanly with the page as flat panels, so that charts feel like part of the structured layout rather than floating glassmorphism panels.

#### Acceptance Criteria

1. THE Chart_Container SHALL use the card/surface color token as its background and the border color token for its 1px border, replacing the current `bg-black/20 backdrop-blur-lg` glassmorphism style. No blur, no transparency.
2. THE Chart_Container SHALL apply a consistent internal padding of 24px (1.5rem) on all sides.
3. WHEN Recharts renders grid lines inside a chart, THE Chart_Container SHALL configure grid line stroke to use the border color token at 60% opacity.
4. WHEN Recharts renders axis tick labels, THE Chart_Container SHALL configure tick fill to use the muted foreground color token.
5. WHEN Recharts renders a tooltip, THE Chart_Container SHALL style the tooltip background with the card color token, a 1px border with the border color token, and text with the foreground color token. No blur on the tooltip.

---

### Requirement 10: Table Visual Design

**User Story:** As a user reviewing run history or project lists, I want tables to be easy to scan row by row in a high-density layout, so that I can find specific entries quickly.

#### Acceptance Criteria

1. THE Table SHALL use the card/surface color token as its background with no transparency or blur.
2. WHEN a table row is hovered, THE Table SHALL apply a background tint using the muted color token at 40–50% opacity.
3. THE Table SHALL render column headers using uppercase letter-spacing, the muted foreground color, a font size of 11–12px, and a thin bottom border using the border color token.
4. THE Table SHALL render row separator lines using the border color token at 1px.
5. WHEN a table cell contains a numeric value (energy, duration, cost), THE Table SHALL apply the monospace font class to that cell.

---

### Requirement 11: Button Visual Design

**User Story:** As a user interacting with the application, I want buttons to follow the W&B style — gold primary buttons with dark text, and outlined secondary buttons — so that interactive affordances are immediately clear.

#### Acceptance Criteria

1. THE Design_System SHALL style primary buttons with the Gold_Accent token as the background color and a dark foreground text color (lightness ≤15%) to achieve a contrast ratio of at least 4.5:1.
2. THE Design_System SHALL style secondary buttons as outlined — transparent background, 1px border using the border color token, and foreground text — with no fill.
3. WHEN a primary button is hovered, THE Design_System SHALL apply a brightness increase of approximately 10% to the Gold_Accent background.
4. WHEN a secondary button is hovered, THE Design_System SHALL apply a subtle background tint using the muted color token.
5. THE Design_System SHALL not use rounded corners larger than 6px on buttons, consistent with the flat W&B panel aesthetic.

---

### Requirement 12: Login and Register Page Visual Design

**User Story:** As a new or returning user, I want the login and register pages to match the redesigned W&B-inspired aesthetic, so that the experience is consistent from first contact.

#### Acceptance Criteria

1. THE Login_Page SHALL use the background color token (Charcoal_Dark neutral gray) as its page background, replacing any current gradient or glow effects.
2. THE Login_Page SHALL render the form in a Flat_Panel card using the card/surface color token and border color token, centered on the page. No blur, no glassmorphism.
3. WHEN a form input is focused, THE Login_Page SHALL apply a ring using the Gold_Accent token.
4. THE Register_Page SHALL apply the same visual treatment as the Login_Page.
5. THE Login_Page SHALL render the submit button as a primary button using the Gold_Accent token per Requirement 11.

---

### Requirement 13: Accessibility — Color and Contrast

**User Story:** As a user with visual sensitivity, I want all text and interactive elements to meet minimum contrast standards, so that the interface is usable without strain.

#### Acceptance Criteria

1. THE Design_System SHALL ensure all normal body text (rendered in near-white primary foreground, approximately `#F5F5F5`) meets WCAG 2.1 AA contrast ratio (4.5:1) against the Charcoal_Dark background surface.
2. THE Design_System SHALL ensure all large text (≥18px regular or ≥14px bold) meets WCAG 2.1 AA contrast ratio (3:1) against its background surface.
3. THE Design_System SHALL ensure all interactive UI components (buttons, links, form inputs) meet WCAG 2.1 AA contrast ratio (3:1) for their visual boundary or label against the adjacent background. WHERE the Gold_Accent token is used as a text or icon color on a Charcoal_Dark surface, THE Design_System SHALL verify the token achieves at least 3:1; if it falls below this threshold, THE Design_System SHALL adjust lightness until the ratio is met.
4. THE Design_System SHALL not rely on color alone to convey state (active, error, success); shape, weight, or iconography SHALL also be used.

---

### Requirement 14: Design Token Centralization

**User Story:** As a developer maintaining the codebase, I want all visual design values to be defined as CSS custom properties in one place, so that future theme changes require editing only one file.

#### Acceptance Criteria

1. THE Design_System SHALL define all color tokens exclusively as CSS custom properties in `app/globals.css` under the `:root` selector.
2. THE Design_System SHALL define all font family tokens as CSS custom properties (`--font-sans`, `--font-mono`) in `app/globals.css`.
3. WHEN a component references a color or font, THE component SHALL reference the CSS custom property token (via Tailwind's `bg-background`, `text-foreground`, `font-mono`, etc.) rather than hardcoding hex or HSL values.
4. IF a component currently hardcodes a color value (e.g., `stroke="rgba(255,255,255,0.1)"`, `text-emerald-400`, `bg-emerald-500/10`), THEN THE component SHALL be updated to reference the appropriate design token or a Tailwind utility derived from it.
