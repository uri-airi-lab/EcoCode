# EcoCode Web Platform

The web platform is the storage, API, and visualisation layer for EcoCode. It consists of:

- **Backend** (`backend/`) — an Axum REST API and SSE server written in Rust, backed by PostgreSQL.
- **Frontend** (`frontend/`) — a Next.js 14 dashboard with real-time charts powered by react-chartjs-2.

---

## Table of Contents

- [Architecture](#architecture)
  - [Backend Module Map](#backend-module-map)
  - [Frontend Module Map](#frontend-module-map)
  - [Real-Time Data Flow (SSE)](#real-time-data-flow-sse)
- [Setup](#setup)
  - [With Docker Compose (recommended)](#with-docker-compose-recommended)
  - [Manual Setup](#manual-setup)
- [Configuration](#configuration)
- [API Reference](#api-reference)
  - [Authentication](#authentication)
  - [Ingestion Endpoints](#ingestion-endpoints)
  - [Query Endpoints](#query-endpoints)
  - [Carbon Configuration](#carbon-configuration)
- [Database Schema](#database-schema)
- [Frontend Pages and Components](#frontend-pages-and-components)
- [Adding a New API Endpoint](#adding-a-new-api-endpoint)
- [TODO / Next Steps](#todo--next-steps)

---

## Architecture

```
Browser (Next.js :3000)
        │
        │  REST + JWT cookie
        │  GET /api/sse (SSE stream)
        ▼
  Axum Backend (:3001)
        │
        │  sqlx (async postgres driver)
        │  tokio::sync::broadcast (SSE fan-out)
        ▼
   PostgreSQL
        ▲
        │  HTTP POST (Bearer API key)
        │
  EcoCode CLI / Python SDK
```

The backend is intentionally stateless (all persistent state lives in PostgreSQL). The only in-memory state is the `tokio::sync::broadcast` channel used to fan live records out to SSE subscribers.

### Backend Module Map

```
web/backend/src/
├── main.rs              # App init: loads .env, creates PG pool, runs schema migrations,
│                        # creates broadcast channel, spawns SSE watcher, registers routes
├── state.rs             # AppState: pool (PgPool), tx (broadcast::Sender<String>), db_url
├── error.rs             # ApiError wrapper: maps Rust errors to HTTP status codes
│
├── auth/
│   ├── mod.rs           # Routes: POST /register, /login, /logout, /change-password
│   │                    # API key routes: POST /api-keys, GET /api-keys, DELETE /api-keys/{id}
│   ├── jwt.rs           # Sign and verify JWT tokens (30-day expiry, HttpOnly cookie)
│   ├── middleware.rs    # AuthUser extractor — accepts JWT cookie or Bearer API key
│   └── password.rs      # Argon2id password hashing; SHA-256 API key hashing
│
├── api/
│   ├── ingest.rs        # POST /ingest/run, /run/{id}/record, /run/{id}/system-info,
│   │                    # /run/{id}/complete — called by CLI and Python SDK
│   ├── runs.rs          # GET /runs, /run/{id}, /run/{id}/record_points,
│   │                    # /run/{id}/summary, /run/{id}/gpu_records, /run/{id}/socket_records
│   │                    # DELETE /run/{id}
│   ├── projects.rs      # GET/POST /projects, DELETE /projects/{id}
│   ├── sse.rs           # GET /sse (SSE stream) + watch_changes() PgListener background task
│   ├── carbon.rs        # GET/PUT /project/{id}/carbon-config,
│   │                    # POST /project/{id}/carbon-config/refresh, GET /carbon-regions
│   └── admin.rs         # GET /admin/projects (admin-only)
│
├── db/
│   ├── schema.rs        # ensure_postgres_schema(): idempotent CREATE TABLE IF NOT EXISTS
│   ├── records.rs       # insert_record_with_details(), fetch_record_rows(),
│   │                    # fetch_record_by_id_with_details(), fetch_gpu_records_for_run()
│   └── runs.rs          # calculate_run_summary_db(): aggregates totals + CO2 + cost
│
├── models/
│   ├── record.rs        # RecordPoint (with nested GpuRecordPoint[], SocketRecordPoint[])
│   ├── run.rs           # Run, RunSummary, AdminRunRow
│   ├── project.rs       # Project data model
│   ├── carbon.rs        # CarbonConfig, CarbonRegion
│   └── ingest.rs        # IngestRunPayload, IngestRecordData, IngestSystemInfo, IngestCompletePayload
│
├── services/
│   └── carbon_api.rs    # Electricity Maps API proxy (fetches live gCO2eq/kWh)
│
└── workspaces/
    ├── mod.rs           # resolve_workspace_id(), require_admin()
    ├── api.rs           # Workspace CRUD + member invitation endpoints
    └── members.rs       # Member role management (admin / member)
```

### Frontend Module Map

```
web/frontend/
├── app/                                    # Next.js App Router
│   ├── layout.tsx                          # Root layout: dark theme, font variables, Toaster
│   ├── page.tsx                            # Root redirect → first workspace
│   ├── login/page.tsx                      # Email/password login form
│   ├── register/page.tsx                   # Registration form (email, institution, position)
│   └── [workspaceSlug]/
│       ├── page.tsx                        # Workspace home: recent runs, summary stats
│       ├── runs/
│       │   ├── page.tsx                    # Paginated run list with filters
│       │   └── [id]/page.tsx               # Live run detail (charts + SSE + polling)
│       ├── projects/
│       │   ├── page.tsx                    # Project list
│       │   └── [id]/page.tsx               # Project detail + run history
│       └── settings/
│           ├── page.tsx                    # Account settings
│           ├── developer/page.tsx          # API key management
│           └── members/page.tsx            # Workspace member management
│
├── components/
│   ├── charts/
│   │   ├── EnergyChart.tsx                 # Cumulative energy over time (line chart)
│   │   ├── EnergyBreakdownChart.tsx        # CPU vs GPU vs MEM vs iGPU split (stacked area)
│   │   ├── UtilizationChart.tsx            # CPU% and GPU% over time
│   │   ├── CumulativePowerChart.tsx        # Running total power draw (Watts)
│   │   ├── PowerDistributionChart.tsx      # Current-interval energy distribution (pie)
│   │   ├── SocketEnergyChart.tsx           # Per-socket RAPL domain breakdown
│   │   ├── PhaseTimeline.tsx               # Training phase timeline (if ML markers present)
│   │   └── TrainingMetricsChart.tsx        # Loss, LR, accuracy trends
│   ├── run/
│   │   ├── RunSystemInfo.tsx               # CPU name, cores, RAM, GPU count and names
│   │   ├── EnvironmentalImpact.tsx         # CO2 with equivalents (car km, tree days)
│   │   ├── MLMetricsBar.tsx                # Current phase, loss, LR, tokens, step
│   │   └── CarbonSettings.tsx             # Per-project intensity, cost, address config
│   ├── tables/
│   │   ├── RunsTable.tsx                   # Paginated runs table with status filters
│   │   ├── ProjectsTable.tsx               # Project list with links
│   │   └── RecentRunsTable.tsx             # Quick summary of latest runs
│   ├── layout/
│   │   ├── DashboardLayout.tsx             # Two-column layout: Sidebar + main content
│   │   ├── Sidebar.tsx                     # Workspace navigation + user dropdown
│   │   └── WorkspaceProvider.tsx           # Global workspace context (projects, user)
│   └── ui/                                 # shadcn/ui base components
│
├── lib/
│   ├── api/
│   │   ├── client.ts                       # apiFetch(): base fetch wrapper with auth headers
│   │   ├── runs.ts                         # fetchRunMeta(), fetchRunRecords(), mapRunSummary()
│   │   ├── projects.ts                     # Project CRUD fetch wrappers
│   │   ├── carbon.ts                       # Carbon config and regions
│   │   └── admin.ts                        # Admin queries
│   ├── mock-data.ts                        # Frontend type definitions (Run, MetricRecord, etc.)
│   ├── utils.ts                            # minIntervalMs(), formatChartTime()
│   ├── chart-colors.ts                     # chartColor() palette for multi-GPU/socket charts
│   └── data-source.ts                      # buildAbsoluteApiUrl()
│
└── hooks/
    └── use-sse.ts                          # SSE subscription hook → live record updates
```

### Real-Time Data Flow (SSE)

Live updates work through two parallel paths:

**Path 1 — Direct broadcast (primary, low latency):**

```
CLI POST /api/ingest/run/{id}/record
  ↓
ingest_record() in ingest.rs
  ↓
state.tx.send(json_payload)          ← broadcasts to all subscribers immediately
  ↓
sse_handler() converts to SSE event
  ↓
Browser EventSource receives it
  ↓
use-sse.ts parses record + appends to chart state
  ↓
Charts re-render
```

**Path 2 — PostgreSQL LISTEN/NOTIFY (fallback for multi-replica deployments):**

```
CLI inserts record → PostgreSQL NOTIFY 'record_inserted' with record_id
  ↓
watch_changes() background task (PgListener)
  ↓
Fetches full record from DB
  ↓
state.tx.send(json_payload)
  ↓
Same SSE path as above
```

The frontend run detail page also does incremental polling (`setInterval`, 1 s) for robustness when SSE is unavailable.

---

## Setup

### With Docker Compose (recommended)

```bash
cd web

# Copy and edit environment file
cp .env.example .env
# Minimum required variables:
#   DATABASE_URL=postgresql://user:pass@db:5432/ecocode
#   CORS_ORIGIN=http://localhost:3000

docker compose up
```

This starts:
- `db` — PostgreSQL 16
- `backend` — Axum API on :3001
- `frontend` — Next.js on :3000

### Manual Setup

**Backend:**

```bash
cd web/backend

# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Set environment variables
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/ecocode"
export CORS_ORIGIN="http://localhost:3000"

# Run
cargo run --release
```

**Frontend:**

```bash
cd web/frontend

# Install pnpm if needed: npm install -g pnpm
pnpm install

# Development mode
NEXT_PUBLIC_API_URL=http://localhost:3001 pnpm dev

# Production build
NEXT_PUBLIC_API_URL=http://localhost:3001 pnpm build
pnpm start
```

---

## Configuration

### Backend Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `CORS_ORIGIN` | No | `http://localhost:3000` | Allowed frontend origin |
| `RUST_LOG` | No | `info` | Log level (`trace`/`debug`/`info`/`warn`/`error`) |

### Frontend Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `NEXT_PUBLIC_API_URL` | No | `http://localhost:3001` | Backend base URL (visible to the browser) |

---

## API Reference

### Authentication

All endpoints (except `/auth/register` and `/auth/login`) require one of:
- A JWT sent as `Authorization: Bearer <token>` (issued by `/auth/login`)
- An API key sent as `Authorization: Bearer eco_...` (created under Settings > Developer)

JWTs are set as 30-day HttpOnly cookies by the login endpoint. The `AuthUser` Axum extractor accepts both.

### Ingestion Endpoints

Called by the EcoCode CLI and Python SDK. Require an API key.

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `POST` | `/api/ingest/run` | `{name, project_id, started_at?, command?, exporter_type?}` | Create a run. Returns `{id}`. |
| `POST` | `/api/ingest/run/{id}/record` | `Record` JSON (see below) | Append one measurement record. |
| `POST` | `/api/ingest/run/{id}/system-info` | `{cpu_name, cpu_cores, ram_gb, gpu_count, gpu_names}` | Save hardware metadata (once per run). |
| `POST` | `/api/ingest/run/{id}/complete` | `{status, ended_at?}` | Finalise run; triggers summary calculation. |

**Record payload structure:**

```json
{
  "pid": 12345,
  "timestamp": "2026-05-01T10:00:01.000Z",
  "cpu_usage": 42.5,
  "cpu_energy": 1.23,
  "gpu_usage": 78.0,
  "gpu_energy": 5.67,
  "mem_usage": 18.3,
  "mem_energy": 0.45,
  "igpu_usage": null,
  "igpu_energy": null,
  "gpu_metrics": [
    {"device_index": 0, "gpu_usage": 78.0, "gpu_energy": 5.67}
  ],
  "socket_metrics": [
    {"socket": 0, "domain": "cpu",  "energy": 1.10},
    {"socket": 0, "domain": "dram", "energy": 0.45}
  ],
  "phase": "forward",
  "tokens_processed": 204800,
  "training_step": 500,
  "loss": 1.23,
  "learning_rate": 0.0003,
  "sequence_length": 2048,
  "accuracy": null,
  "custom_metrics": null
}
```

All energy fields are in Joules (Watt-seconds).

### Query Endpoints

Called by the frontend. Require a JWT or API key.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/runs` | All runs for the authenticated user with summaries (paginated) |
| `GET` | `/api/project/{id}/runs` | Runs for a specific project |
| `GET` | `/api/run/{id}` | Single run metadata |
| `GET` | `/api/run/{id}/record_points` | All records with nested GPU + socket details. Supports `?limit=N&offset=M` |
| `GET` | `/api/run/{id}/summary` | Cached aggregate summary (totals, CO2, cost) |
| `GET` | `/api/run/{id}/gpu_records` | All GPU metrics for a run |
| `GET` | `/api/run/{id}/socket_records` | All socket metrics for a run |
| `DELETE` | `/api/run/{id}` | Delete a run and all its records |
| `GET` | `/api/projects` | All projects in the user's workspaces |
| `POST` | `/api/projects` | Create a new project |
| `DELETE` | `/api/projects/{id}` | Delete a project |
| `GET` | `/api/sse` | SSE stream — live record updates |

### Carbon Configuration

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/project/{id}/carbon-config` | Get carbon and cost settings for a project |
| `PUT` | `/api/project/{id}/carbon-config` | Update manual intensity, cost, address, API key |
| `POST` | `/api/project/{id}/carbon-config/refresh` | Fetch live intensity from Electricity Maps API |
| `GET` | `/api/carbon-regions` | List all 50+ static carbon regions with intensities |

---

## Database Schema

The schema is applied idempotently at startup via `ensure_postgres_schema()`. Energy values are stored in Joules throughout. Timestamps are stored as `TIMESTAMPTZ`.

```sql
-- Identity & access
users (id, email, password_hash, full_name, institution, position, use_case, created_at)
workspaces (id, name, slug, created_at)
workspace_members (workspace_id, user_id, role, joined_at)
api_keys (id, user_id, workspace_id, name, key_hash, created_at)

-- Experiments
projects (id, workspace_id, name, collaborator_name, country_code, address,
          latitude, longitude, electricity_maps_api_key, manual_cost_per_kwh,
          use_api_for_cost, created_at)

runs (id, name, project_id, collaborator_name, status, started_at, ended_at,
      command, exporter_type)

run_system_info (run_id, cpu_name, cpu_cores, ram_gb, gpu_count, gpu_names)

-- Time-series records
records (id, run_id, pid, timestamp, cpu_usage, cpu_energy, gpu_usage, gpu_energy,
         mem_usage, mem_energy, igpu_usage, igpu_energy,
         phase, tokens_processed, training_step, loss, learning_rate,
         sequence_length, accuracy, custom_metrics)

gpu_records (id, record_id, device_index, gpu_usage, gpu_energy)
socket_records (id, record_id, socket, domain, energy)

-- Aggregates (calculated on run completion)
runsummary (run_id, total_cpu_energy, total_gpu_energy, total_mem_energy,
            total_igpu_energy, total_co2_g, carbon_intensity_used, pue_used,
            total_cost, currency_used, avg_cpu_usage, avg_gpu_usage, avg_mem_usage)

-- Carbon configuration
project_carbon_config (project_id, manual_cost_per_kwh, use_api_for_cost,
                       resolved_intensity_g_per_kwh, intensity_source)
```

---

## Frontend Pages and Components

### Run Detail Page (`app/[workspaceSlug]/runs/[id]/page.tsx`)

The most complex page. Its loading strategy:

1. **Phase 1** — fetch run metadata and cached summary immediately. Renders the header, system info, and environmental impact panel. Charts show a loading spinner.
2. **Phase 2** — fetch all historical records in the background with up to 3 retries (exponential backoff). When records arrive, charts populate and `chartsLoading` becomes false.
3. **Live polling** — if `run.status === 'running'`, a `setInterval` at 1 s fetches only new records (`?offset=N`) and appends them. A slower 5 s interval re-fetches run metadata to detect status changes.
4. **SSE** — (`use-sse.ts`) subscribes to `/api/sse` for near-zero-latency chart updates when the CLI is actively writing records.

### Chart Components

All charts use react-chartjs-2 and receive `records: MetricRecord[]` as their primary prop.

| Component | Chart type | X-axis | Y-axis |
|-----------|-----------|--------|--------|
| `EnergyChart` | Line | Time | Cumulative energy (Wh) per component |
| `EnergyBreakdownChart` | Stacked area | Time | CPU / GPU / MEM / iGPU split |
| `UtilizationChart` | Line | Time | CPU% and GPU% |
| `CumulativePowerChart` | Line | Time | Running total power (W) |
| `PowerDistributionChart` | Doughnut | — | Latest-interval energy share |
| `SocketEnergyChart` | Bar | Socket+domain | Cumulative energy per RAPL domain |
| `PhaseTimeline` | Custom bar | Time | Phase label bands |
| `TrainingMetricsChart` | Line | Step | Loss, learning rate, accuracy |

### `use-sse.ts` Hook

```typescript
// Subscribes to /api/sse on mount. Returns { latestRecord, error }.
// On each SSE message, parses the JSON payload and returns it as MetricRecord.
// Auto-reconnects on disconnect. Cleans up on unmount.
const { latestRecord } = useSSE()
```

Charts on the run detail page subscribe to this hook and append `latestRecord` to their local state on every update, enabling true real-time visualisation.

---

## Adding a New API Endpoint

1. **Define the handler** in `src/api/<module>.rs`:

```rust
pub async fn my_handler(
    auth_user: auth::AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<MyPayload>,
) -> Result<impl IntoResponse, ApiError> {
    // ... query the DB via state.pool ...
    Ok(Json(result))
}
```

2. **Register the route** in `src/main.rs`:

```rust
.route("/api/my-endpoint", post(api::my_module::my_handler))
```

3. **Add models** in `src/models/` if the handler needs new request/response types.

4. **Add a fetch wrapper** in `web/frontend/lib/api/`:

```typescript
export async function myFetch(payload: MyPayload): Promise<MyResponse> {
    return apiFetch<MyResponse>('/api/my-endpoint', {
        method: 'POST',
        body: JSON.stringify(payload),
    })
}
```

---