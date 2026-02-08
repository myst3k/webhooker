# Webhooker — Review TODO

## 🔴 CRITICAL

- [ ] **Email action module is a stub** — `src/actions/email.rs:89-95` — `load_tenant_smtp()` hardcoded to return Err. Module needs DB pool access to load tenant SMTP configs.
- [ ] **Login rate limiter counts all attempts** — `src/rate_limit.rs:56-73` — Counter increments before password check. Only count failures.
- [ ] **CORS headers missing on POST responses** — `src/routes/ingest.rs` — OPTIONS handler returns CORS but POST handler doesn't. Browser fetch() calls blocked.

## 🟠 HIGH

- [ ] **No action queue — actions run synchronously** — No action_queue migration, no worker loop. Submissions block on external calls. Need Postgres-backed queue per DESIGN.md.
- [ ] **Webhook SSRF** — `src/actions/webhook.rs` — No URL validation. Block private/reserved IPs and metadata endpoints.
- [ ] **Template injection** — `src/actions/template.rs` — User data interpolated into HTML without escaping.
- [ ] **No body size limit enforced** — `config.max_body_size` parsed but never applied as middleware. Add `RequestBodyLimitLayer`.

## 🟡 MEDIUM

- [ ] **Registration race condition** — `src/routes/auth.rs:60-62` — Two concurrent first-user registrations could both become system admin. Use DB lock or unique constraint.
- [ ] **Export query not tenant-scoped** — `src/db/submissions.rs:95-102` — Route defends it but query should also scope by tenant for defense-in-depth.
- [ ] **Crypto key uses raw SHA-256** — `src/crypto.rs:6-10` — Use HKDF or Argon2 for key derivation instead.
- [ ] **Rate limit cleanup never called** — `src/rate_limit.rs` — DashMaps grow unbounded. Add periodic Tokio cleanup task.
- [ ] **Cookie security flags missing** — access_token cookie not set with HttpOnly/Secure/SameSite.
- [ ] **System admin can delete own tenant** — `src/routes/admin.rs:63-74` — Add guard to prevent self-destruction.
- [ ] **Sort column injection fragile** — `src/db/submissions.rs:46-48` — Replace format!() SQL interpolation with enum.
- [ ] **Missing action_queue migration** — Add table per DESIGN.md.

## 🔵 LOW

- [ ] **Forgot-password untracked task** — `src/routes/auth.rs:140` — tokio::spawn without JoinHandle tracking.
- [ ] **Docker healthcheck needs curl** — `docker-compose.yml:30` — Image doesn't include curl. Use different check or install it.
- [ ] **Weak default secrets** — `docker-compose.yml:22-23` — Fail startup if JWT_SECRET or ENCRYPTION_KEY are default values.
- [ ] **No email format validation** — `src/routes/auth.rs` — Login/register accept any string as email.
- [ ] **Slugify duplicated 3x** — `src/routes/projects.rs`, `src/routes/endpoints.rs`, `src/routes/auth.rs` — Extract to shared utility.
- [ ] **No updated_at on users** — `migrations/20250101000002_users.sql` — Hard to know when user was last modified.
- [ ] **Cargo.lock glob in Dockerfile** — `Dockerfile:4` — Require Cargo.lock explicitly for reproducible builds.

## ℹ️ INFO (non-actionable notes)

- JWT uses HS256 — fine for single-server, upgrade to EdDSA if needed later
- No Discord/Slack action modules yet — just email + webhook for now
- HTMX partial routes exist — verify submissions table partial works
- No request tracing middleware — consider adding `tower_http::TraceLayer`
