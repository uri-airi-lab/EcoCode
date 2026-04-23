# Plan: Add Carbon Emission Calculation to EcoCode

## Context

EcoCode already measures energy consumption per process (CPU, GPU, RAM) in Watts, and the frontend has a **hardcoded** world-average carbon factor (`CARBON_FACTOR = 0.475 kgCO2/kWh`) in `web/frontend/lib/api-client.ts`. There is already an `EnvironmentalImpact` component. However:

- The carbon factor is hardcoded — no user configuration for country/region
- CO2 values are computed only in the frontend, never stored in the database
- There is no PUE (Power Usage Effectiveness) support for data center workloads

**Goal:** Upgrade carbon tracking to a first-class, configurable, database-backed feature with:
- **Real-time** carbon intensity via Electricity Maps API (lat/lon → gCO2eq/kWh)
- **Address geocoding** via OpenStreetMap Nominatim (free, no API key needed) to convert address → lat/lon
- **Static fallback** lookup table (~23 countries) when no API key is configured
- PUE factor support for data center workloads

---

## Standard Formula

```
CO₂ (g) = Energy (Wh) / 1000 × Carbon_Intensity (gCO₂eq/kWh) × PUE
```

Parameters:
- **Energy (Wh):** Sum of **all four** `runsummary` fields: `total_cpu_energy + total_gpu_energy + total_mem_energy + total_igpu_energy`
  - `total_cpu_energy` — CPU cores (RAPL `cpu` domain)
  - `total_gpu_energy` — discrete NVIDIA GPU (NVML)
  - `total_mem_energy` — DRAM (RAPL `dram` domain)
  - `total_igpu_energy` — integrated GPU (RAPL `igpu`/`uncore` domain, may be NULL if absent)
  - NULL components are treated as 0 (e.g., no iGPU on a machine with discrete GPU only)
- **Carbon Intensity:** Real-time from Electricity Maps API, or static fallback by country
- **PUE:** Power Usage Effectiveness — 1.0 for local workstation, 1.58 for average data center

---

## APIs Used

### 1. OpenStreetMap Nominatim (Geocoding — free, no key)
- Converts address → lat/lon
- Endpoint: `https://nominatim.openstreetmap.org/search?q={address}&format=json`
- Returns: `lat`, `lon`, `display_name`
- Rate limit: 1 req/sec (sufficient for one-time config)

### 2. Electricity Maps API (Real-time carbon intensity — free tier)
- Endpoint: `GET https://api.electricitymap.org/v3/carbon-intensity/latest?lat={lat}&lon={lon}`
- Headers: `auth-token: {api_key}`
- Returns: `carbonIntensity` (gCO2eq/kWh), `zone`, `datetime`
- Free tier: 50 requests/hour, 1 zone — sufficient for one-time or periodic refresh
- Global coverage: 190+ countries

### 3. Static fallback table (built-in, no API key required)
- Used when no Electricity Maps API key is configured
- ~23 country entries (IEA/carbonfootprint.com 2024/2025 data)

---

## Implementation Plan

### Step 1 — Create a shared carbon module

**New file:** `ecocode/src/carbon.rs`

```rust
pub const CARBON_REGIONS: &[(&str, &str, f64)] = &[
    ("WORLD", "Global Average", 494.0),
    ("FR", "France", 52.0),
    ("SE", "Sweden", 13.0),
    ("NO", "Norway", 28.0),
    ("CH", "Switzerland", 43.0),
    ("AT", "Austria", 158.0),
    ("GB", "United Kingdom", 207.0),
    ("ES", "Spain", 163.0),
    ("IT", "Italy", 233.0),
    ("NL", "Netherlands", 295.0),
    ("BE", "Belgium", 167.0),
    ("DE", "Germany", 380.0),
    ("PL", "Poland", 635.0),
    ("US", "United States", 367.0),
    ("CA", "Canada", 130.0),
    ("BR", "Brazil", 91.0),
    ("MX", "Mexico", 415.0),
    ("AU", "Australia", 510.0),
    ("JP", "Japan", 471.0),
    ("KR", "South Korea", 415.0),
    ("CN", "China", 555.0),
    ("IN", "India", 680.0),
    ("ZA", "South Africa", 700.0),
];

pub fn static_intensity(country_code: &str) -> Option<f64> { ... }

pub fn calculate_co2_g(energy_wh: f64, intensity_g_per_kwh: f64, pue: f64) -> f64 {
    energy_wh / 1000.0 * intensity_g_per_kwh * pue
}
```

This module is shared by both the core tool and the web backend (via a workspace or copy).

### Step 2 — Database schema additions

**Files to modify:**
- `ecocode/src/exporter/local.rs`
- `ecocode/src/exporter/online.rs`
- `web/backend/src/main.rs`

**New `carbon_config` table (SQLite):**
```sql
CREATE TABLE IF NOT EXISTS carbon_config (
    id INTEGER PRIMARY KEY,
    -- Location (used to fetch real-time intensity)
    address TEXT,               -- Human-readable address (display only)
    latitude REAL,              -- For Electricity Maps API call
    longitude REAL,
    -- API key for Electricity Maps (optional)
    electricity_maps_api_key TEXT,
    -- Resolved intensity (cached from last API call or static lookup)
    resolved_intensity_g_per_kwh REAL NOT NULL DEFAULT 494.0,
    intensity_source TEXT NOT NULL DEFAULT 'static_world',  -- 'api' | 'static_{code}'
    intensity_fetched_at TEXT,  -- timestamp of last API fetch
    -- Static fallback country (used when no API key)
    country_code TEXT NOT NULL DEFAULT 'WORLD',
    -- PUE factor
    pue_factor REAL NOT NULL DEFAULT 1.0,
    updated_at TEXT NOT NULL
);
INSERT OR IGNORE INTO carbon_config (id, country_code, pue_factor, updated_at)
VALUES (1, 'WORLD', 1.0, datetime('now'));
```

**Add to `runsummary`:**
```sql
-- Add with ALTER TABLE ... ADD COLUMN IF NOT EXISTS for migrations
ALTER TABLE runsummary ADD COLUMN total_co2_g REAL;
ALTER TABLE runsummary ADD COLUMN carbon_intensity_used REAL;  -- gCO2/kWh used
ALTER TABLE runsummary ADD COLUMN pue_used REAL;               -- PUE factor at time of run
```

### Step 3 — Web backend: geocoding + Electricity Maps integration

**File to modify:** `web/backend/src/main.rs`

**New dependency** in `web/backend/Cargo.toml`:
```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
```

**New API endpoints:**

```
GET  /api/carbon-config
     → Returns full carbon_config row + resolved_intensity_g_per_kwh

PUT  /api/carbon-config
     Body: {
       address?: string,          -- triggers geocode → Electricity Maps lookup
       latitude?: number,         -- alternative to address
       longitude?: number,
       country_code?: string,     -- static fallback
       electricity_maps_api_key?: string,
       pue_factor?: number
     }
     → Geocodes address (if provided) via Nominatim
     → Fetches real-time intensity from Electricity Maps (if api_key set)
     → Falls back to static lookup by country_code
     → Saves everything to carbon_config table

POST /api/carbon-config/refresh-intensity
     → Re-fetches intensity from Electricity Maps using stored lat/lon + api_key
     → Returns updated intensity

GET  /api/carbon-regions
     → Returns static lookup table: [{ code, name, intensity_g_per_kwh }]
```

**Internal helper functions in main.rs:**
```rust
async fn geocode_address(address: &str) -> Result<(f64, f64, String)>
// → calls Nominatim, returns (lat, lon, display_name)

async fn fetch_electricity_maps(lat: f64, lon: f64, api_key: &str) -> Result<f64>
// → calls Electricity Maps API, returns gCO2eq/kWh
```

**Update existing endpoint:**
- `GET /api/run/{run_id}/summary` → include `total_co2_g`, `carbon_intensity_used`, `pue_used`

### Step 4 — Core tool: calculate and store CO2 in run summary

**Files to modify:**
- `ecocode/src/exporter/mod.rs` — add `total_co2_g`, `carbon_intensity_used`, `pue_used` to `RunSummary`
- `ecocode/src/exporter/local.rs` — read `carbon_config`, compute + store CO2 in `run_summary()`
- `ecocode/src/exporter/online.rs` — same for PostgreSQL

**Logic in `run_summary()`:**
```rust
// Read from carbon_config table
let config = db.query_one("SELECT resolved_intensity_g_per_kwh, pue_factor FROM carbon_config WHERE id=1");

// Energy totals already computed in summary — include ALL measured components
let energy_wh = (total_cpu
    + total_gpu
    + total_mem.unwrap_or(0.0)   // DRAM (RAPL dram domain)
    + total_igpu.unwrap_or(0.0)  // iGPU (RAPL igpu/uncore domain)
) / 3600.0; // Watt-seconds → Wh

let co2_g = calculate_co2_g(energy_wh, config.intensity, config.pue);

// Store back in runsummary
db.execute("UPDATE runsummary SET total_co2_g=?, carbon_intensity_used=?, pue_used=? WHERE run_id=?",
    co2_g, config.intensity, config.pue, run_id);
```

### Step 5 — Frontend: settings UI + updated dashboard

**Files to modify:**

1. **`web/frontend/lib/api-client.ts`**
   - Remove `CARBON_FACTOR = 0.475`
   - Add functions: `fetchCarbonConfig()`, `updateCarbonConfig(payload)`, `refreshCarbonIntensity()`, `fetchCarbonRegions()`
   - Update `RunSummary` type to include `total_co2_g`, `carbon_intensity_used`, `pue_used`

2. **`web/frontend/components/CarbonSettings.tsx`** (**NEW**)
   - Address input field → on blur: calls `PUT /api/carbon-config` with `{ address }`
     - Backend geocodes and optionally fetches real-time intensity
     - Returns resolved lat/lon + intensity
   - OR: country dropdown for static fallback (from `/api/carbon-regions`)
   - Electricity Maps API key input (optional, stored in DB)
   - PUE factor slider (1.0 – 3.0, step 0.1)
   - "Refresh intensity" button → calls `POST /api/carbon-config/refresh-intensity`
   - Shows: "Current intensity: 52 gCO₂/kWh (France · Electricity Maps · 14 min ago)"

3. **`web/frontend/components/EnvironmentalImpact.tsx`**
   - Use `total_co2_g` from run summary (DB value) as source of truth
   - Primary metric: CO₂ in grams (or kg if > 1000g)
   - Equivalents:
     - Trees needed to absorb (1 tree/year ≈ 21,000 gCO₂)
     - Car kilometers driven (1 km ≈ 120 gCO₂)
     - Smartphone charges (1 charge ≈ 8.2 gCO₂)
   - Show intensity source: "Based on 52 gCO₂/kWh (France)"
   - Graceful fallback for old runs (null `total_co2_g`): recalculate client-side using `energy × 494`

4. **`web/frontend/app/page.tsx`** (or a new settings page)
   - Add Carbon Settings panel (collapsible accordion or settings drawer)
   - Show current carbon intensity badge near run summary header

---

## Critical Files

| File | Change |
|------|--------|
| `ecocode/src/carbon.rs` | **NEW** — static table + `calculate_co2_g()` |
| `ecocode/src/exporter/mod.rs` | Add CO₂ fields to RunSummary |
| `ecocode/src/exporter/local.rs` | `carbon_config` table, CO₂ in `run_summary()` |
| `ecocode/src/exporter/online.rs` | Same for PostgreSQL |
| `web/backend/Cargo.toml` | Add `reqwest` |
| `web/backend/src/main.rs` | Schema, 4 new endpoints, geocode + API helpers |
| `web/frontend/lib/api-client.ts` | Remove hardcoded factor, add carbon API calls |
| `web/frontend/components/EnvironmentalImpact.tsx` | Use DB value, richer CO₂ display |
| `web/frontend/components/CarbonSettings.tsx` | **NEW** — address/region/PUE settings |

---

## Carbon Intensity Data (embedded in `carbon.rs`)

| Code | Country | gCO₂eq/kWh | Source |
|------|---------|------------|--------|
| WORLD | Global Average | 494 | IEA 2025 |
| FR | France | 52 | carbonfootprint.com 2025 |
| SE | Sweden | 13 | carbonfootprint.com 2025 |
| NO | Norway | 28 | carbonfootprint.com 2025 |
| CH | Switzerland | 43 | carbonfootprint.com 2025 |
| AT | Austria | 158 | carbonfootprint.com 2025 |
| GB | United Kingdom | 207 | carbonfootprint.com 2025 |
| ES | Spain | 163 | carbonfootprint.com 2025 |
| IT | Italy | 233 | carbonfootprint.com 2025 |
| NL | Netherlands | 295 | carbonfootprint.com 2025 |
| BE | Belgium | 167 | carbonfootprint.com 2025 |
| DE | Germany | 380 | carbonfootprint.com 2025 |
| PL | Poland | 635 | carbonfootprint.com 2025 |
| US | United States | 367 | carbonfootprint.com 2025 |
| CA | Canada | 130 | carbonfootprint.com 2025 |
| BR | Brazil | 91 | carbonfootprint.com 2025 |
| MX | Mexico | 415 | carbonfootprint.com 2025 |
| AU | Australia | 510 | carbonfootprint.com 2025 |
| JP | Japan | 471 | carbonfootprint.com 2025 |
| KR | South Korea | 415 | carbonfootprint.com 2025 |
| CN | China | 555 | carbonfootprint.com 2025 |
| IN | India | 680 | carbonfootprint.com 2025 |
| ZA | South Africa | 700 | carbonfootprint.com 2025 |

---

## Trade-offs & Notes

- **Address vs coordinates:** Nominatim geocodes the address for free (no API key). The lat/lon is stored and reused for subsequent Electricity Maps queries so geocoding only runs once per address change.
- **API key optional:** Without an Electricity Maps key, the system falls back gracefully to the static lookup by country. With a key, it gets hourly real-time intensity.
- **Caching intensity:** The resolved intensity is stored in `carbon_config.resolved_intensity_g_per_kwh` with a timestamp. The `/refresh-intensity` button lets the user update it on demand. Alternatively, the backend could refresh automatically every hour.
- **Backward compat:** Old `runsummary` rows with NULL `total_co2_g` are handled in the frontend by falling back to `energy × 494 gCO₂/kWh`.
- **Privacy:** The address/lat/lon is stored locally in the SQLite DB only — never sent anywhere except the geocoding and carbon intensity APIs.

## Verification

1. `cargo build` in `ecocode/` and `web/backend/` — no errors
2. Set address via `PUT /api/carbon-config` with `{ "address": "Paris, France" }`
3. Verify response contains `latitude ≈ 48.85`, `longitude ≈ 2.35`, `resolved_intensity_g_per_kwh ≈ 52` (static fallback without API key)
4. Add Electricity Maps API key → re-PUT config → verify `intensity_source = 'api'`
5. Run `ecocode run -- sleep 10` → check `runsummary.total_co2_g` is populated
6. Open web dashboard → CarbonSettings shows address + intensity
7. EnvironmentalImpact component shows CO₂ from DB, not recalculated
