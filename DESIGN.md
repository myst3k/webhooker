# Webhooker — Self-hosted Submission Catcher

Open source, multi-tenant submission endpoint. Not a form builder — just catch, store, notify.

Accepts POST data from anywhere: HTML forms, scripts, webhooks, IoT, whatever. Defines expected fields optionally, captures everything regardless.

**Repo:** `myst3k/webhooker`
**Stack:** Rust + Axum + Postgres 18 + Askama + HTMX + Pico CSS
**License:** Dual MIT / Apache 2.0

---

## Hierarchy

```
Tenant (isolated sandbox)
  └── Project (logical grouping)
        └── Endpoint (submission target)
              └── Submission (captured data)
```

---

## Tenants

Tenants are **isolated sandboxes**. Not SaaS multi-tenancy — just "give someone else their own space on your instance."

- First user registers → default tenant auto-created, user is system admin + tenant owner
- System admin creates additional tenants + users as needed
- Each user belongs to **one tenant** — no multi-tenant membership, no workspace switching
- Registration is **closed by default** — system admin creates all accounts
- Every DB query scoped by `tenant_id` from JWT. No exceptions.

**Use case:** You're running this at work. A coworker wants to use it. You create them a tenant and account. They see only their stuff, can't see or mess with yours.

---

## User Model

```
System Admin (platform owner, first user)
  ├── Tenant: yours
  │     ├── You (owner)
  │     └── Your teammate (member)
  │
  └── Tenant: coworker's sandbox
        └── Coworker (owner)
```

**Roles:**

| Role | Scope | Can do |
|------|-------|--------|
| system_admin | platform | everything + create tenants/users, peek into any tenant |
| owner | tenant | everything in their tenant + manage members |
| member | tenant | CRUD on assigned projects, view submissions |

### System Admin Can
- Create / delete tenants
- Create users and assign to any tenant
- Disable / delete any user
- Reset any user's password
- View all tenants + users
- Peek into any tenant's data for support

### Tenant Owner Can
- Add members to their tenant (creates new account)
- Remove members from their tenant
- Change member roles
- Reset member passwords
- Manage all projects/endpoints in their tenant

### Members Can
- CRUD on projects and endpoints within their tenant
- View submissions
- Configure actions on their endpoints
- Cannot manage other users

---

## Authentication

### Passwords
- **Argon2id** (memory: 19MB, iterations: 2, parallelism: 1)
- Never store plaintext, never log passwords

### Tokens
- **Access token (JWT):** 15 minute expiry, signed with EdDSA or HS256
- **Refresh token:** 7 day expiry, stored as sha256 hash in DB, rotated on every use
- **Refresh reuse detection:** If a previously-used refresh token is presented → nuke ALL refresh tokens for that user (compromise signal)

### JWT Payload
```json
{
  "sub": "user-uuidv7",
  "tid": "tenant-uuidv7",
  "role": "owner",
  "sys": false,
  "exp": 1707350400
}
```

### Auth Endpoints
```
POST /api/v1/auth/register          → first user only (bootstrap), then disabled
POST /api/v1/auth/login             → returns { access_token, refresh_token }
POST /api/v1/auth/refresh           → rotates refresh token, returns new pair
POST /api/v1/auth/logout            → deletes refresh token
POST /api/v1/auth/forgot-password   → sends reset email (always returns 200)
POST /api/v1/auth/reset-password    → validates token, updates password, nukes all refresh tokens
```

### Password Reset Flow
1. User submits email to `/forgot-password`
2. System generates token, stores sha256 hash in DB with 1hr expiry
3. Sends email via **system SMTP** with reset link
4. Always returns 200 — never reveal whether email exists
5. User clicks link, submits new password + token to `/reset-password`
6. Token validated (expiry + single use), password updated, all refresh tokens revoked

### Brute Force Protection
- Rate limit login attempts: 5 per email per 15 minutes
- Return same error for wrong email vs wrong password ("invalid credentials")

---

## Data Model

All primary keys use **Postgres 18 native `uuidv7()`** — time-ordered, sortable, no index fragmentation, no extensions needed.

### tenants
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK, DEFAULT uuidv7() |
| name | varchar(255) | |
| slug | varchar(100) | unique, URL-friendly |
| created_at | timestamptz | DEFAULT now() |
| updated_at | timestamptz | DEFAULT now() |

### users
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| tenant_id | uuidv7 | FK → tenants |
| email | varchar(255) | unique |
| password_hash | varchar(255) | argon2id |
| name | varchar(255) | |
| role | varchar(20) | owner, member |
| is_system_admin | bool | default false |
| created_at | timestamptz | |

### refresh_tokens
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| user_id | uuidv7 | FK → users |
| token_hash | varchar(64) | sha256 of token |
| used | bool | default false (for reuse detection) |
| expires_at | timestamptz | |
| created_at | timestamptz | |

### password_reset_tokens
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| user_id | uuidv7 | FK → users |
| token_hash | varchar(64) | sha256 |
| used | bool | default false |
| expires_at | timestamptz | 1 hour from creation |
| created_at | timestamptz | |

### tenant_smtp_configs
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| tenant_id | uuidv7 | FK → tenants, unique |
| host | varchar(255) | |
| port | int | |
| username_enc | bytea | AES-256-GCM encrypted |
| password_enc | bytea | AES-256-GCM encrypted |
| from_address | varchar(255) | |
| from_name | varchar(255) | optional |
| tls_mode | varchar(10) | starttls, tls, none |
| created_at | timestamptz | |
| updated_at | timestamptz | |

### projects
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| tenant_id | uuidv7 | FK → tenants |
| name | varchar(255) | |
| slug | varchar(100) | unique per tenant |
| created_at | timestamptz | |
| updated_at | timestamptz | |

### endpoints
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| project_id | uuidv7 | FK → projects |
| name | varchar(255) | e.g., "Contact Submissions" |
| slug | varchar(100) | unique per project |
| fields | jsonb | optional — expected field definitions |
| settings | jsonb | CORS, rate limit, honeypot, retention |
| created_at | timestamptz | |
| updated_at | timestamptz | |

### submissions
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| endpoint_id | uuidv7 | FK → endpoints |
| data | jsonb | fields that matched defined fields |
| extras | jsonb | fields that didn't match |
| raw | jsonb | untouched original payload |
| metadata | jsonb | IP, user-agent, referrer |
| created_at | timestamptz | |

### actions
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| endpoint_id | uuidv7 | FK → endpoints |
| action_type | varchar(50) | email, webhook, discord, slack |
| config | jsonb | module-specific settings |
| position | int | execution order |
| enabled | bool | default true |
| created_at | timestamptz | |

### action_log
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| action_id | uuidv7 | FK → actions |
| submission_id | uuidv7 | FK → submissions |
| status | varchar(20) | success, failed, skipped |
| response | jsonb | status code, body, error message |
| executed_at | timestamptz | |

### audit_events
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| tenant_id | uuidv7 | FK |
| user_id | uuidv7 | FK, nullable (system events) |
| action | varchar(50) | endpoint.created, user.login, etc. |
| resource_type | varchar(50) | endpoint, project, user |
| resource_id | uuidv7 | |
| details | jsonb | old/new values, IP, context |
| created_at | timestamptz | |

---

## Submission Processing

### Incoming Request
```
POST /v1/e/{endpoint_id}
Content-Type: application/json | application/x-www-form-urlencoded | multipart/form-data
```

### Processing Pipeline

1. **Rate limit check** — sliding window per IP per endpoint
2. **Parse body** — support JSON, form-urlencoded, multipart
3. **Honeypot check** — if honeypot field is configured and filled → reject silently (200 OK, don't store)
4. **Store raw** — save entire payload untouched to `raw`
5. **Sort fields:**
   - If endpoint has defined fields → matched keys go to `data`, unmatched go to `extras`
   - If no fields defined → everything goes to `data`, `extras` is empty
6. **Validate** — run type checks on matched fields (email format, required, etc.) — warn but don't reject
7. **Capture metadata** — IP (respect trusted proxies), user-agent, referrer, timestamp
8. **Store submission**
9. **Run action pipeline** — execute each enabled action in position order
10. **Log action results**
11. **Respond** — 201 Created (JSON) or redirect (if configured + form POST)

### Field Definitions (endpoints.fields)
```json
[
  { "name": "email", "type": "email", "required": true, "label": "Email Address" },
  { "name": "name", "type": "text", "required": true, "label": "Full Name" },
  { "name": "message", "type": "textarea", "required": false, "label": "Message" }
]
```

Supported types: `text`, `email`, `phone`, `textarea`, `number`, `url`, `select`, `checkbox`

### Endpoint Settings (endpoints.settings)
```json
{
  "cors_origins": ["https://example.com"],
  "rate_limit": 10,
  "rate_limit_window_secs": 60,
  "honeypot_field": "_gotcha",
  "store_metadata": true,
  "redirect_url": "https://example.com/thanks",
  "retention_days": null
}
```

---

## Action Module System

Each action type is a pluggable Rust module implementing a trait:

```rust
#[async_trait]
pub trait ActionModule: Send + Sync {
    /// Unique identifier: "email", "webhook", "discord"
    fn id(&self) -> &str;

    /// Human-readable name for the UI
    fn name(&self) -> &str;

    /// JSON schema describing this module's config
    fn config_schema(&self) -> serde_json::Value;

    /// Validate config before saving
    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ActionError>;

    /// Execute the action
    async fn execute(
        &self,
        ctx: &ActionContext,
        config: &serde_json::Value,
    ) -> Result<ActionResult, ActionError>;
}

pub struct ActionContext {
    pub submission: Submission,
    pub endpoint: Endpoint,
    pub project: Project,
    pub tenant: Tenant,
}

pub struct ActionResult {
    pub status: ActionStatus, // Success, Failed
    pub response: Option<serde_json::Value>,
}
```

### Module Registry
```rust
pub struct ModuleRegistry {
    modules: HashMap<String, Arc<dyn ActionModule>>,
}
```

All modules register at startup. `GET /api/v1/modules` returns available action types + their config schemas (UI uses this to render config forms dynamically).

### Pipeline Execution
Actions execute in `position` order. Each action is independent — one failure doesn't stop the rest. All results logged to `action_log`.

### MVP Modules

| Module | Crate | Purpose |
|--------|-------|---------|
| `email` | `lettre` | SMTP notification with template |
| `webhook` | `reqwest` | Generic HTTP POST to any URL |
| `discord` | `reqwest` | Discord webhook with formatted embed |
| `slack` | `reqwest` | Slack webhook with Block Kit |

### Template Variables
Actions support basic interpolation:
- `{{data.name}}` — matched field value
- `{{extras.utm_source}}` — extra field value
- `{{endpoint.name}}` — endpoint name
- `{{project.name}}` — project name
- `{{submission.created_at}}` — timestamp
- `{{metadata.ip}}` — submitter IP

### Future Modules
- Telegram
- Google Sheets append
- S3/Wasabi file upload
- Forward to another Webhooker instance
- Conditional execution (filter expressions)
- WASM plugins (any language)

---

## Email System

Two completely separate email paths:

### System Emails (Transactional)
Configured once by system admin via environment variables. Used for:
- Password reset links
- Account creation notifications ("Welcome to Webhooker")
- Member added to tenant notifications
- Security alerts (optional)

**Templates** — Askama, compiled into binary:
- `welcome.html` — account created
- `password_reset.html` — reset link (1hr expiry)
- `member_added.html` — "You've been added to {{tenant.name}}"

System SMTP config is env-var only. Not stored in DB, not accessible to tenants.

### Action Emails (Per-Tenant SMTP)
Each tenant configures their own SMTP credentials (stored encrypted in `tenant_smtp_configs`). When an endpoint's email action fires, it uses the **tenant's SMTP**, not the system SMTP.

- Tenant owner configures SMTP in their settings page
- Credentials encrypted at rest (AES-256-GCM, key from `WEBHOOKER_ENCRYPTION_KEY` env var)
- If tenant has no SMTP configured → email actions fail with clear error in action log
- **No fallback to system SMTP** — tenant submission notifications never go through the system admin's mail server

This means each tenant controls their own sender address, deliverability, and reputation.

### API for Tenant SMTP
```
GET    /api/v1/tenant/smtp          → current config (credentials masked)
PUT    /api/v1/tenant/smtp          → set/update SMTP config
DELETE /api/v1/tenant/smtp          → remove SMTP config
POST   /api/v1/tenant/smtp/test     → send test email to verify config
```

---

## Action Queue (Postgres-backed)

Submissions return immediately after storing data. Actions execute asynchronously via a Postgres-backed queue.

### Flow
1. `POST /v1/e/{endpoint_id}` → parse, validate, store submission
2. For each enabled action on the endpoint → `INSERT INTO action_queue (status='pending')`
3. Return `201 Created` — submitter never waits on actions
4. Background worker (Tokio task, same process) picks up pending items and executes

### action_queue table
| Column | Type | Notes |
|--------|------|-------|
| id | uuidv7 | PK |
| submission_id | uuidv7 | FK → submissions |
| action_id | uuidv7 | FK → actions |
| status | varchar(20) | pending, processing, completed, failed |
| attempts | int | default 0 |
| max_attempts | int | default 3 |
| last_error | text | nullable |
| next_retry_at | timestamptz | for backoff scheduling |
| created_at | timestamptz | |
| completed_at | timestamptz | nullable |

### Worker Loop
```sql
SELECT * FROM action_queue
WHERE status = 'pending' AND next_retry_at <= now()
ORDER BY created_at
LIMIT 10
FOR UPDATE SKIP LOCKED
```

`FOR UPDATE SKIP LOCKED` enables scaling to multiple workers later without double-processing.

### Retry Strategy
- Attempt 1 fails → retry after 30s
- Attempt 2 fails → retry after 2min
- Attempt 3 fails → mark as `failed`, log to `action_log`

### On Completion
- Success → status = `completed`, write to `action_log`, set `completed_at`
- Final failure → status = `failed`, write to `action_log` with error details

### Queue Cleanup
Background task: delete `completed` entries older than 7 days. `failed` entries kept 30 days for debugging.

### Dashboard Indicators
Each endpoint shows queue health:
```
Submissions: 1,247
Actions: ✅ 3,720 completed | ⏳ 2 pending | ❌ 5 failed
```

System admin gets global queue health: total pending, failure rate, oldest pending item.

---

## Dashboard (Askama + HTMX + Pico CSS)

Single binary serves both API and UI. No separate frontend build.

**Pico CSS** — classless/minimal CSS framework via CDN. Semantic HTML looks good out of the box. Dark mode built in.

**HTMX** — dynamic UI via HTML fragment swapping. Search, filter, pagination, modals — no JavaScript framework.

### Pages

| Route | View |
|-------|------|
| `/` | Login / redirect to dashboard |
| `/dashboard` | Projects overview |
| `/projects/{slug}` | Endpoints list |
| `/endpoints/{slug}` | Submissions table |
| `/endpoints/{slug}/settings` | Endpoint config + fields |
| `/endpoints/{slug}/actions` | Action pipeline config |
| `/endpoints/{slug}/snippet` | Copy-paste integration code |
| `/admin/tenants` | Tenant management (system admin) |
| `/admin/users` | User management (system admin) |
| `/settings` | Account settings, password change |
| `/settings/smtp` | Tenant SMTP configuration |
| `/settings/members` | Tenant member management (owner) |
| `/auth/forgot-password` | Password reset request |
| `/auth/reset-password` | Password reset form |

### Submission Table Features
- Columns from defined fields + auto-discovered keys
- Sort by any column
- Filter/search across submissions
- Expand row to see extras + raw + metadata
- Toggle column visibility
- Export filtered results: CSV, JSON
- Bulk delete / bulk mark as read
- Pagination via HTMX (swap table body)

### Integration Snippets
Each endpoint auto-generates copy-paste code:
- Plain HTML form
- JavaScript fetch
- curl command
- Python requests

---

## API Routes

### Public (submissions)
```
POST   /v1/e/{endpoint_id}              → accept submission
```

### Auth
```
POST   /api/v1/auth/register            → bootstrap first user
POST   /api/v1/auth/login               → get tokens
POST   /api/v1/auth/refresh             → rotate refresh token
POST   /api/v1/auth/logout              → revoke refresh token
```

### Projects
```
GET    /api/v1/projects                  → list
POST   /api/v1/projects                  → create
GET    /api/v1/projects/{id}             → get
PUT    /api/v1/projects/{id}             → update
DELETE /api/v1/projects/{id}             → delete
```

### Endpoints
```
GET    /api/v1/projects/{id}/endpoints   → list
POST   /api/v1/projects/{id}/endpoints   → create
GET    /api/v1/endpoints/{id}            → get
PUT    /api/v1/endpoints/{id}            → update
DELETE /api/v1/endpoints/{id}            → delete
```

### Submissions
```
GET    /api/v1/endpoints/{id}/submissions          → list (paginated, filterable)
GET    /api/v1/endpoints/{id}/submissions/export    → CSV or JSON export
GET    /api/v1/submissions/{id}                     → get single
DELETE /api/v1/submissions/{id}                     → delete
DELETE /api/v1/endpoints/{id}/submissions           → bulk delete (with filter)
```

### Actions
```
GET    /api/v1/endpoints/{id}/actions    → list actions for endpoint
POST   /api/v1/endpoints/{id}/actions    → add action
PUT    /api/v1/actions/{id}              → update action
DELETE /api/v1/actions/{id}              → delete action
GET    /api/v1/actions/{id}/log          → action execution history
```

### Modules
```
GET    /api/v1/modules                   → list available action modules + config schemas
```

### Admin (system admin only)
```
GET    /api/v1/admin/tenants             → list all tenants
POST   /api/v1/admin/tenants             → create tenant
GET    /api/v1/admin/tenants/{id}        → get tenant details
DELETE /api/v1/admin/tenants/{id}        → delete tenant + all data
GET    /api/v1/admin/users               → list all users
POST   /api/v1/admin/users               → create user (assign to tenant)
DELETE /api/v1/admin/users/{id}          → delete user
```

### Tenant (owner scope)
```
GET    /api/v1/tenant                    → current tenant info
PUT    /api/v1/tenant                    → update tenant
GET    /api/v1/tenant/members            → list members
POST   /api/v1/tenant/members            → add member (creates account)
PUT    /api/v1/tenant/members/{id}       → update member role
DELETE /api/v1/tenant/members/{id}       → remove member
POST   /api/v1/tenant/members/{id}/reset-password → reset member's password
```

### Tenant SMTP (owner scope)
```
GET    /api/v1/tenant/smtp              → current config (credentials masked)
PUT    /api/v1/tenant/smtp              → set/update SMTP config
DELETE /api/v1/tenant/smtp              → remove SMTP config
POST   /api/v1/tenant/smtp/test         → send test email to verify config
```

---

## Security

### Submission Endpoint
- Rate limiting per IP per endpoint (configurable, default 10/min)
- Request body size limit (default 1MB)
- CORS allowlist per endpoint (no config = accept all)
- Honeypot field — auto-reject if filled (silent 200)
- UUIDv7 endpoint IDs are unguessable (74 bits random)
- No authentication required — the endpoint ID is the identifier

### Dashboard / API
- Argon2id password hashing
- JWT with short expiry (15min)
- Refresh token rotation with reuse detection
- Brute force protection (5 attempts / 15min per email)
- Tenant isolation enforced at middleware level — every query scoped by tenant_id
- Same error for wrong email vs wrong password

### Infrastructure
- System secrets via environment variables (JWT key, system SMTP creds, encryption key)
- Tenant SMTP credentials encrypted at rest (AES-256-GCM)
- Audit log for all mutations
- Optional data retention policy per endpoint (auto-purge)
- Trusted proxy configuration for accurate IP capture
- Single `pg_dump` backs up everything (tenant SMTP creds remain encrypted in dump)

---

## Configuration (Environment Variables)

```bash
# Required
DATABASE_URL=postgres://user:pass@host:5432/webhooker
JWT_SECRET=your-secret-key
WEBHOOKER_ENCRYPTION_KEY=your-256-bit-key  # AES-256-GCM for tenant SMTP creds

# System SMTP (transactional emails — password resets, account notifications)
WEBHOOKER_SMTP_HOST=smtp.example.com
WEBHOOKER_SMTP_PORT=587
WEBHOOKER_SMTP_USER=system@example.com
WEBHOOKER_SMTP_PASS=app-password
WEBHOOKER_SMTP_FROM=noreply@example.com

# NOTE: Tenant/action SMTP is configured per-tenant in the DB, NOT here.

# Optional
WEBHOOKER_HOST=0.0.0.0
WEBHOOKER_PORT=3000
WEBHOOKER_BASE_URL=https://webhooker.example.com  # for password reset links
WEBHOOKER_REGISTRATION=closed          # closed | open
WEBHOOKER_MAX_BODY_SIZE=1048576        # bytes (1MB)
WEBHOOKER_TRUSTED_PROXIES=10.0.0.0/8   # for X-Forwarded-For
WEBHOOKER_LOG_LEVEL=info
```

---

## Tech Stack Summary

| Layer | Choice | Why |
|-------|--------|-----|
| Language | Rust | Performance, safety, single binary |
| Web framework | Axum | Async, tower middleware, ergonomic |
| Database | Postgres 18 | Native uuidv7(), jsonb, battle-tested |
| ORM/Query | SQLx | Compile-time checked queries, async |
| Templates | Askama | Compile into binary, type-safe |
| Interactivity | HTMX | Dynamic UI without JS framework |
| CSS | Pico CSS | Classless, CDN, dark mode, zero build |
| Password hashing | argon2 crate | Argon2id |
| JWT | jsonwebtoken crate | Industry standard |
| HTTP client | reqwest | For webhook/action modules |
| Email | lettre | SMTP |
| Rate limiting | governor | In-memory sliding window |
| Serialization | serde + serde_json | |

---

## MVP Scope

1. Auth: register (bootstrap), login, JWT + refresh tokens
2. Tenant auto-creation on first register
3. System admin: create tenants + users
4. CRUD: projects, endpoints
5. Submission endpoint: accept POST, sort fields, store
6. Action pipeline: email + webhook modules
7. Dashboard: view submissions table, expand details, export CSV
8. Integration snippet generation
9. Rate limiting + honeypot
10. Audit log

### Post-MVP
- Discord + Slack action modules
- Bulk operations on submissions
- Conditional action execution (filters)
- Data retention auto-purge
- Submission search (full-text on jsonb)
- Dashboard charts (submission volume over time)
- WASM plugin support
- API key auth option for server-to-server use
- Docker image + Helm chart

---

## Deployment

- **Docker:** Single-stage build, `FROM scratch` or `distroless`
- **k8s:** Deployment + Service + Ingress
- **CI:** GitHub Actions, image tags `main-YYYYMMDDHHmmss` + `latest`
- **Database:** Postgres 18 (separate deployment or managed)
- **Migrations:** SQLx embedded migrations, run on startup
