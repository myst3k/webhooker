# ⚡ Webhooker

A self-hosted submission endpoint. Not a form builder — just catch, store, notify.

Webhooker accepts POST data from anywhere — HTML forms, scripts, webhooks, IoT devices, cron jobs — and stores it. Define expected fields or don't. It captures everything regardless, sorts what it recognizes, and runs your notification pipeline.

## Why

You have a contact form. Or a landing page. Or a script that needs to POST somewhere. You don't want to wire up a database, email notifications, and a dashboard every time. You just want an endpoint that catches submissions and tells you about them.

That's Webhooker. One endpoint, one POST, done.

## Features

- **Accept anything** — JSON, form-urlencoded, multipart. From any source.
- **Smart field sorting** — Define expected fields and incoming data gets sorted into matched (`data`), unmatched (`extras`), and a raw copy of the original payload.
- **Action pipeline** — Pluggable notification modules that fire on every submission. Email, webhooks, Discord, Slack — or write your own.
- **Multi-tenant isolation** — Give others their own sandbox on your instance. They can't see your data, you can't accidentally see theirs (unless you're the admin).
- **Built-in dashboard** — View, filter, search, and export submissions. No separate frontend to deploy.
- **Self-hosted** — Single binary + Postgres. Your data stays on your server.

## Quick Start

### Docker Compose

```bash
git clone https://github.com/myst3k/webhooker.git
cd webhooker
cp .env.example .env
# Edit .env — change JWT_SECRET and WEBHOOKER_ENCRYPTION_KEY
docker compose up
```

Open [http://localhost:3000](http://localhost:3000) and register your admin account.

### From Source

Requires Rust (2024 edition) and Postgres 18.

```bash
git clone https://github.com/myst3k/webhooker.git
cd webhooker
cp .env.example .env
# Edit .env with your database URL and secrets
cargo run
```

## How It Works

### 1. Create an endpoint

After logging in, create a project and add an endpoint. Optionally define expected fields:

```json
[
  { "name": "email", "type": "email", "required": true },
  { "name": "name", "type": "text", "required": true },
  { "name": "message", "type": "textarea", "required": false }
]
```

### 2. POST to it

```bash
curl -X POST https://your-instance.com/v1/e/{endpoint_id} \
  -H "Content-Type: application/json" \
  -d '{"name": "Jane", "email": "jane@example.com", "message": "Hello", "utm_source": "google"}'
```

Or use a plain HTML form:

```html
<form action="https://your-instance.com/v1/e/{endpoint_id}" method="POST">
  <input type="text" name="name" required>
  <input type="email" name="email" required>
  <textarea name="message"></textarea>
  <input type="hidden" name="_gotcha" style="display:none">
  <button type="submit">Send</button>
</form>
```

### 3. Data gets sorted

```
Incoming: { "name": "Jane", "email": "jane@example.com", "message": "Hello", "utm_source": "google" }

→ data:   { "name": "Jane", "email": "jane@example.com", "message": "Hello" }
→ extras: { "utm_source": "google" }
→ raw:    (original payload, untouched)
```

If no fields are defined, everything goes to `data`.

### 4. Actions fire

Configure notification actions per endpoint — email alerts, webhook forwarding, Discord/Slack messages. Actions run asynchronously; the submission response returns immediately.

## Action Modules

Webhooker uses a pluggable module system. Built-in modules:

| Module | What it does |
|--------|-------------|
| **Email** | SMTP notification (per-tenant SMTP config) |
| **Webhook** | Forward submission to any URL via HTTP POST |

Each tenant configures their own SMTP credentials — your notification emails come from your mail server, not anyone else's.

Writing a custom module is straightforward — implement the `ActionModule` trait and register it.

## Multi-Tenancy

Webhooker supports isolated tenants for shared instances. This isn't SaaS multi-tenancy — it's "give someone else their own space."

- **System admin** creates tenants and user accounts
- Each user belongs to one tenant
- Tenants are fully isolated — separate projects, endpoints, submissions, and SMTP configs
- Registration is closed by default — the admin controls who gets access

```
You (system admin)
├── Your Tenant
│   └── Project: Marketing Site
│       └── Endpoint: Contact Form
│
└── Dave's Tenant
    └── Project: Campaign Pages
        └── Endpoint: Newsletter Signup
```

Dave sees only his stuff.

## Configuration

All configuration via environment variables:

```bash
# Required
DATABASE_URL=postgres://user:pass@host:5432/webhooker
JWT_SECRET=your-random-secret
WEBHOOKER_ENCRYPTION_KEY=your-32-char-encryption-key

# System SMTP (for password resets, account notifications)
WEBHOOKER_SMTP_HOST=smtp.example.com
WEBHOOKER_SMTP_PORT=587
WEBHOOKER_SMTP_USER=system@example.com
WEBHOOKER_SMTP_PASS=app-password
WEBHOOKER_SMTP_FROM=noreply@example.com

# Optional
WEBHOOKER_HOST=0.0.0.0                    # Listen address
WEBHOOKER_PORT=3000                        # Listen port
WEBHOOKER_BASE_URL=https://webhooker.example.com
WEBHOOKER_REGISTRATION=closed              # closed | open
WEBHOOKER_MAX_BODY_SIZE=1048576            # Max request body (bytes)
WEBHOOKER_TRUSTED_PROXIES=10.0.0.0/8       # For X-Forwarded-For
WEBHOOKER_LOG_LEVEL=info
```

Tenant SMTP is configured per-tenant in the dashboard, not in environment variables.

## API

### Submission (public)
```
POST /v1/e/{endpoint_id}
```

### Dashboard API (authenticated)
```
# Auth
POST   /api/v1/auth/login
POST   /api/v1/auth/refresh
POST   /api/v1/auth/logout

# Projects & Endpoints
GET    /api/v1/projects
POST   /api/v1/projects
GET    /api/v1/projects/{id}/endpoints
POST   /api/v1/projects/{id}/endpoints

# Submissions
GET    /api/v1/endpoints/{id}/submissions
GET    /api/v1/endpoints/{id}/submissions/export?format=csv

# Actions
GET    /api/v1/endpoints/{id}/actions
POST   /api/v1/endpoints/{id}/actions
GET    /api/v1/modules
```

Full API documentation: see [DESIGN.md](DESIGN.md)

## Tech Stack

| | |
|---|---|
| **Language** | Rust |
| **Framework** | Axum |
| **Database** | PostgreSQL 18 (native UUIDv7) |
| **Templates** | Askama |
| **Interactivity** | HTMX |
| **Auth** | Argon2id + JWT |
| **Encryption** | AES-256-GCM (tenant SMTP credentials) |

## Anti-Spam

- **Honeypot fields** — configurable per endpoint, silent rejection
- **Rate limiting** — per IP per endpoint, configurable limits
- **CORS restrictions** — optional origin allowlist per endpoint

## Security

- Argon2id password hashing
- JWT access tokens (15min) with refresh token rotation
- Refresh token reuse detection (compromise signal → revoke all)
- Tenant isolation enforced at the database query level
- Tenant SMTP credentials encrypted at rest (AES-256-GCM)
- Audit logging for all mutations
- Brute force protection on login

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
