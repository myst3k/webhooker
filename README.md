# ⚡ Webhooker

A self-hosted submission endpoint. Not a form builder — just catch, store, notify.

Webhooker accepts POST data from anywhere — HTML forms, scripts, webhooks, IoT devices, cron jobs — and stores it. Define expected fields or don't. It captures everything regardless, sorts what it recognizes, and fires your notification pipeline.

## Why

You have a contact form. Or a landing page. Or a script that needs to POST somewhere. You don't want to wire up a database, email notifications, and a dashboard every time. You just want an endpoint that catches submissions and tells you about them.

That's Webhooker. One endpoint, one POST, done.

## Features

- **Accept anything** — JSON, form-urlencoded, multipart. From any source.
- **Smart field sorting** — Define expected fields and incoming data gets sorted into matched (`data`), unmatched (`extras`), and a raw copy of the original payload. Or define nothing and everything goes to `data`.
- **Action pipeline** — Pluggable notification modules that fire on every submission. Email and webhook built in.
- **Multi-tenant isolation** — Give others their own sandbox on your instance. Fully isolated projects, endpoints, and submissions.
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

Open [http://localhost:3000](http://localhost:3000) and register your admin account (first registration only).

### From Source

Requires Rust and Postgres 18+.

```bash
git clone https://github.com/myst3k/webhooker.git
cd webhooker
cp .env.example .env
# Edit .env with your database URL and secrets
cargo run
```

## How It Works

### 1. Create an endpoint

Log in, create a project, add an endpoint. Optionally define the fields you expect:

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

Or a plain HTML form:

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

No fields defined? Everything goes to `data`.

### 4. View in the dashboard

Submissions show up in a searchable, filterable table. Expand any row to see the full payload, extras, and metadata (IP, user-agent, referrer). Export to CSV or JSON.

## Action Modules

Configure actions per endpoint — they fire on every new submission.

| Module | Status | Description |
|--------|--------|-------------|
| **Webhook** | ✅ Ready | Forward submissions to any URL via HTTP POST |
| **Email** | 🚧 WIP | SMTP notifications using per-tenant SMTP config |
| **Discord** | 📋 Planned | Post to Discord webhooks |
| **Slack** | 📋 Planned | Post to Slack webhooks |

The module system is pluggable — implement the `ActionModule` trait to add your own.

## Multi-Tenancy

Tenants are isolated sandboxes on a shared instance. Not SaaS — just "give someone their own space."

- System admin creates tenants and user accounts
- Each user belongs to one tenant
- Tenants can't see each other's data
- Registration is closed by default

## Configuration

```bash
# Required
DATABASE_URL=postgres://user:pass@host:5432/webhooker
JWT_SECRET=your-random-secret
WEBHOOKER_ENCRYPTION_KEY=your-32-char-encryption-key

# System SMTP (password resets, account notifications)
WEBHOOKER_SMTP_HOST=smtp.example.com
WEBHOOKER_SMTP_PORT=587
WEBHOOKER_SMTP_USER=system@example.com
WEBHOOKER_SMTP_PASS=app-password
WEBHOOKER_SMTP_FROM=noreply@example.com

# Optional
WEBHOOKER_HOST=0.0.0.0
WEBHOOKER_PORT=3000
WEBHOOKER_BASE_URL=https://webhooker.example.com
WEBHOOKER_REGISTRATION=closed
WEBHOOKER_MAX_BODY_SIZE=1048576
WEBHOOKER_TRUSTED_PROXIES=10.0.0.0/8
WEBHOOKER_LOG_LEVEL=info
```

Tenant SMTP is configured per-tenant in the dashboard settings.

## Anti-Spam

- **Honeypot fields** — configurable per endpoint, silent rejection
- **Rate limiting** — per IP per endpoint
- **CORS restrictions** — optional origin allowlist per endpoint

## Tech Stack

| | |
|---|---|
| **Language** | Rust |
| **Framework** | Axum |
| **Database** | PostgreSQL 18 |
| **Templates** | Askama + HTMX |
| **Auth** | Argon2id + JWT |

## Roadmap

See [DESIGN.md](DESIGN.md) for the full design spec and [TODO.md](TODO.md) for known issues.

- [ ] Async action queue (Postgres-backed, currently synchronous)
- [ ] Email action module completion (tenant SMTP loading)
- [ ] Discord and Slack action modules
- [ ] Conditional action execution (filters)
- [ ] Submission search (full-text)
- [ ] Data retention auto-purge

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
