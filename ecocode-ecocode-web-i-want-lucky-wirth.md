# EcoCode: PostgreSQL-Only Multi-Tenant Platform Refactor

## Context

The web backend currently runs a dual-database architecture: every handler accepts a `?source=local|online` query param and branches between a SQLite pool and an optional PostgreSQL pool, maintaining two copies of every SQL query (one with `?` placeholders, one with `$N`). The goal is to collapse this to a single PostgreSQL pool, remove the source-toggle system entirely from both backend and frontend, improve the auth/workspace system, and keep SQLite only in the local CLI (`ecocode/` directory, untouched).

---

## Phase 1 — Backend: Cargo + Schema

**File:** `web/backend/Cargo.toml`
- Remove `"sqlite"` from sqlx features: `features = ["postgres", "macros", "runtime-tokio-rustls", "chrono"]`

**File:** `web/backend/.env.example`
```
DATABASE_URL=postgres://user:pass@localhost/ecocode
JWT_SECRET=your_super_secret_jwt_key_here
PRODUCTION=false
CORS_ORIGIN=http://localhost:3000
```

**File:** `web/backend/src/main.rs` — schema section
- Delete `ensure_sqlite_schema`, `ensure_sqlite_column`, `has_sqlite_column`, `backfill_sqlite_project_collaborators`, `sync_sqlite_project_shortcuts` (~330 lines)
- In `ensure_postgres_schema`, add workspace_id to api_keys:
  ```rust
  ensure_postgres_column(&pool, "api_keys", "workspace_id",
      "ALTER TABLE api_keys ADD COLUMN workspace_id BIGINT REFERENCES workspaces(id) ON DELETE SET NULL").await?;
  ```

---

## Phase 2 — Backend: AppState + main() (`main.rs`)

**Remove entirely:**
- `DataSource` enum
- `SourceQuery` struct
- `DataSourceResponse` struct
- `available_sources()`, `resolve_source()`, `resolve_default_source()` fns
- `get_data_sources` handler
- `resolve_local_db_url()`, `resolve_online_db_url()` fns
- Dual `watch_source_changes` — replace with single watcher
- SQLite pool init in `main()`
- `tx_local` / `tx_online` dual channels

**New AppState:**
```rust
#[derive(Clone)]
pub(crate) struct AppState {
    pool: PgPool,
    tx: broadcast::Sender<String>,
}
```

**New `main()`:**
```rust
let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
let pool = PgPoolOptions::new().max_connections(10).connect(&db_url).await?;
ensure_postgres_schema(&pool).await?;
let (tx, _) = broadcast::channel::<String>(100);
let state = AppState { pool, tx: tx.clone() };
tokio::spawn(watch_changes(state.clone()));
let cors_origin = std::env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
```

**CORS:** Move origin to env var `CORS_ORIGIN` (remove hardcoded `http://localhost:3000`).

**Remove** `/api/data-sources` route registration.

**`Project` struct:** Add `workspace_slug: String` field.

**`fetch_projects` query** — add JOIN to return slug:
```sql
SELECT p.id, p.name, w.slug AS workspace_slug
FROM projects p
JOIN workspace_members wm ON p.workspace_id = wm.workspace_id
JOIN workspaces w ON p.workspace_id = w.id
WHERE wm.user_id = $1 ORDER BY p.name ASC
```

**All `fetch_*` helper functions:** Collapse `match source { Local => ..., Online => ... }` to single body using `$N` placeholders and `&state.pool`. The PostgreSQL branch SQL is correct — just remove the match and the Local branch.

**All handlers:** Remove `Query(source_query): Query<SourceQuery>` param and `let source = resolve_source(...)` line from every handler. Affected: `get_projects`, `post_projects`, `get_runs`, `get_run`, `get_record_points`, `get_gpu_records`, `get_socket_records`, `get_run_summary`, `get_admin_projects`, `get_carbon_config_handler`, `put_carbon_config_handler`, `post_refresh_carbon_intensity`.

**`post_projects`:** Change `_auth_user` → `auth_user` and add workspace membership check:
```rust
let membership = sqlx::query("SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
    .bind(payload.workspace_id).bind(auth_user.user_id)
    .fetch_optional(&state.pool).await.map_err(ApiError::internal)?;
if membership.is_none() {
    return Err(ApiError::new(StatusCode::FORBIDDEN, "You are not a member of this workspace"));
}
```

**Pagination on `get_runs`:** Add `PaginationQuery { limit: Option<i64>, offset: Option<i64> }`. Append `LIMIT $N OFFSET $N+1` to `fetch_runs` (defaults: 100/0).

**Single SSE watcher:**
```rust
async fn watch_changes(state: AppState) {
    let mut last_id = 0_i64;
    loop {
        if let Ok(Some(r)) = fetch_latest_record_with_details(&state).await {
            if r.id > last_id {
                if let Ok(payload) = serde_json::to_string(&r) { let _ = state.tx.send(payload); }
                last_id = r.id;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
```

---

## Phase 3 — Backend: auth.rs + workspaces.rs

### auth.rs

**`AuthUser` struct:**
```rust
pub(crate) struct AuthUser {
    pub user_id: i64,
    pub workspace_id: Option<i64>,  // set when authed via scoped API key
}
```

**`from_request_parts`:** Remove source resolution entirely. Use `state.pool` directly:
```rust
// API key lookup:
let row = sqlx::query("SELECT user_id, workspace_id FROM api_keys WHERE key_hash = $1")
    .bind(&key_hash).fetch_optional(&app_state.pool).await...;
// JWT path: unchanged logic, just no source check
```

**Secure cookie:** Use env var at call time:
```rust
fn is_production() -> bool { std::env::var("PRODUCTION").as_deref() == Ok("true") }
// Cookie builder: .secure(is_production())
```

**All auth handlers** (`register`, `login`, `create_api_key`, `list_api_keys`, `delete_api_key`): Remove `Query(source_query)` param and `match auth_user.source` branching. Single query with `&state.pool`.

**`CreateApiKeyPayload`:** Add `workspace_id: Option<i64>`.
**INSERT:** `INSERT INTO api_keys (user_id, name, key_hash, workspace_id) VALUES ($1, $2, $3, $4) RETURNING id, created_at::text`
**`ApiKeyResponse`:** Add `workspace_id: Option<i64>`.

### workspaces.rs

Remove `DataSource` import and all `match auth_user.source` branching. Use `&state.pool` with `$N` throughout.

**Add new routes:**
```rust
.route("/{slug}/members", get(list_members))
.route("/{slug}/members/{user_id}", delete(remove_member))
```

**`list_members`:** Require admin role. Query:
```sql
SELECT wm.user_id, u.email, wm.role, wm.joined_at::text
FROM workspace_members wm JOIN users u ON wm.user_id = u.id
WHERE wm.workspace_id = $1 ORDER BY wm.joined_at ASC
```
Returns `Vec<MemberResponse { user_id, email, role, joined_at }>`.

**`remove_member`:** Require admin. Block self-removal (400). `DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2`. Returns 204.

**Invite duplicate fix:** Already exists in the Online branch — after collapsing, the 409 "User is already a member" path is preserved automatically.

---

## Phase 4 — Frontend: Remove DataSource System

### `lib/data-source.ts` — gut to minimal form:
```typescript
export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || ''

export function buildApiUrl(path: string): string {
  if (API_BASE_URL) return new URL(path, API_BASE_URL).toString()
  return path
}
```
Delete: `DataSource`, `DataSourceAvailability`, `normalizeDataSource`, `readStoredDataSource`, `storeDataSource`, `fetchDataSourceAvailability`.

### `components/DataSourceProvider.tsx` — delete entire file.

### `lib/api-client.ts`:
- Change import: `import { buildApiUrl } from './data-source'`
- Remove `source?: DataSource` from ALL exported + internal functions
- Replace `buildApiUrl(path, source)` → `buildApiUrl(path)` everywhere
- Add `workspace_slug: string` to `BackendProject` interface

### `app/layout.tsx` — remove `<DataSourceProvider>` wrapper and its import.

### `components/Sidebar.tsx`:
- Remove `useDataSource` import and destructure
- Change workspace fetch to plain `fetch('/api/workspaces')`
- Remove the entire Local/Online toggle button group (the `grid grid-cols-2 gap-2 px-2` div)

### `components/StartRunModal.tsx`:
- Remove `useDataSource` import and `const { source } = useDataSource()`
- Change workspace fetch to plain `fetch('/api/workspaces')`
- Remove `source` from all API calls (`fetchCarbonConfig`, `createProject`, `updateCarbonConfig`)

### `components/CarbonSettings.tsx`:
- Remove `useDataSource` import + usage
- Remove `source` from `fetchCarbonConfig`, `updateCarbonConfig`, `refreshCarbonIntensity` calls

### All pages using `useDataSource` — remove import, destructure, and `source` from every API call:
- `app/[workspaceSlug]/page.tsx`
- `app/[workspaceSlug]/projects/page.tsx`
- `app/[workspaceSlug]/projects/[id]/page.tsx`
- `app/[workspaceSlug]/runs/page.tsx`
- `app/[workspaceSlug]/runs/[id]/page.tsx`
- `app/admin/page.tsx`

### `hooks/use-sse.ts`:
- Remove `DataSource` import and `source` parameter
- Signature: `export function useSSE()`
- URL: `buildApiUrl('/api/sse')` (no source param)
- Remove `source` from `useEffect` dep array
- Update all call sites: `useSSE(source)` → `useSSE()`

---

## Phase 5 — Frontend: New Features

### New file: `components/WorkspaceProvider.tsx`

Global context that fetches workspaces once and resolves current workspace from the URL. Exposes `{ workspaces, currentWorkspace, loading, refresh }`.

```typescript
'use client'
// Uses usePathname() to derive slug from URL
// currentWorkspace = workspaces.find(w => w.slug === slug) ?? null
export function WorkspaceProvider({ children }) { ... }
export function useWorkspace() { ... }
```

Add `<WorkspaceProvider>` to `app/layout.tsx`.

Update `Sidebar.tsx` and `StartRunModal.tsx` to use `useWorkspace()` instead of their local `fetch('/api/workspaces')` effects — eliminates duplicated network calls.

### `app/[workspaceSlug]/settings/members/page.tsx` — expand members UI:

Add a members list table above the invite form:
- On mount: `GET /api/workspaces/:slug/members`
- Columns: Email | Role | Joined | Actions
- Admin-only "Remove" button per row (hidden for current user's own row)
- Uses `currentWorkspace.role === 'admin'` from `useWorkspace()` to decide visibility
- On remove: `DELETE /api/workspaces/:slug/members/:user_id`, then refresh list

---

## Suggestions (beyond the stated scope)

1. **Workspace-scoped API keys**: CLI can sync to a specific workspace by generating a key with `workspace_id`. The AuthUser extractor will carry `workspace_id` and future sync endpoints can use it for routing without asking the user to specify a workspace ID in every request.

2. **CORS env var**: Moving `CORS_ORIGIN` to env prevents production deployments requiring recompile just to change the allowed origin.

3. **Secure cookie in production**: Setting `.secure(true)` only when `PRODUCTION=true` prevents confusing localhost dev failures while ensuring cookies are protected in prod.

4. **`workspace_slug` on projects**: Frontend can derive workspace context from the project response itself, removing the need to cross-reference a separate workspaces fetch in several places.

5. **Pagination on runs**: Long-running projects will accumulate hundreds of runs; returning all of them in one call will become slow. Defaulting to 100/page costs nothing now and prevents a later breaking change.

6. **Membership enforcement on `POST /api/projects`**: Currently any authenticated user can create a project in any workspace by guessing its ID. The explicit member check closes this gap.

---

## Verification

1. `cargo build` in `web/backend/` — must compile with no SQLite references
2. `cargo run` — starts with `DATABASE_URL` pointing to a test Postgres instance, logs "PostgreSQL pool initialized"
3. `POST /api/auth/register` + `POST /api/auth/login` — returns JWT + sets cookie
4. `POST /api/workspaces` → `GET /api/workspaces` → verify workspace returned
5. `POST /api/workspaces/:slug/invite` with existing member → expect 409
6. `GET /api/workspaces/:slug/members` (admin) → returns member list
7. `DELETE /api/workspaces/:slug/members/:id` → returns 204
8. `POST /api/projects` with non-member workspace_id → expect 403
9. `POST /api/auth/api-keys` with `workspace_id` → verify `workspace_id` in response
10. Frontend: `npm run build` in `web/frontend/` — must compile with no `DataSource` / `useDataSource` references
11. Visit dashboard — no Local/Online toggle visible in sidebar
12. Run creation, carbon config, SSE live updates — all work without `?source=` params

---

## Critical Files

| File | Change |
|------|--------|
| `web/backend/Cargo.toml` | Remove sqlite feature |
| `web/backend/src/main.rs` | AppState, all handlers, schema, watcher |
| `web/backend/src/auth.rs` | AuthUser, all auth handlers |
| `web/backend/src/workspaces.rs` | Workspace handlers + new member routes |
| `web/backend/.env.example` | New env vars |
| `web/frontend/lib/data-source.ts` | Strip to buildApiUrl only |
| `web/frontend/lib/api-client.ts` | Remove source params |
| `web/frontend/components/DataSourceProvider.tsx` | Delete |
| `web/frontend/components/Sidebar.tsx` | Remove toggle |
| `web/frontend/app/layout.tsx` | Swap DataSourceProvider → WorkspaceProvider |
| `web/frontend/components/StartRunModal.tsx` | Remove source usage |
| `web/frontend/components/CarbonSettings.tsx` | Remove source usage |
| `web/frontend/hooks/use-sse.ts` | Remove source param |
| `web/frontend/app/[workspaceSlug]/*/page.tsx` (×5) | Remove useDataSource |
| `web/frontend/app/admin/page.tsx` | Remove useDataSource |
| `web/frontend/components/WorkspaceProvider.tsx` | **New file** |
| `web/frontend/app/[workspaceSlug]/settings/members/page.tsx` | Expand members UI |
