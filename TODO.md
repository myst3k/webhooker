# Webhooker — Review TODO

## 🟠 HIGH

- [x] **No action queue — actions run synchronously** — Postgres-backed queue with dedicated worker pool.
- [x] **Webhook SSRF** — URL validation with configurable strict/relaxed mode and CIDR allowlist.
- [x] **Template injection** — HTML-escaped interpolation for HTML email bodies.
- [x] **No body size limit enforced** — `RequestBodyLimitLayer` applied globally.

## 🟡 MEDIUM

- [ ] **Registration race condition** — `src/routes/auth.rs:60-62` — Two concurrent first-user registrations could both become system admin. Use DB lock or unique constraint.
- [x] **Export query not tenant-scoped** — Added tenant_id join to list_for_export query.
- [x] **Crypto key uses raw SHA-256** — Replaced with HKDF-SHA256 key derivation.
- [ ] **Rate limit cleanup never called** — `src/rate_limit.rs` — DashMaps grow unbounded. Add periodic Tokio cleanup task.
- [ ] **Cookie security flags missing** — access_token cookie not set with HttpOnly/Secure/SameSite.
- [ ] **System admin can delete own tenant** — `src/routes/admin.rs:63-74` — Add guard to prevent self-destruction.
- [ ] **Sort column injection fragile** — `src/db/submissions.rs:46-48` — Replace format!() SQL interpolation with enum.
- [x] **Missing action_queue migration** — Added with async worker pool.

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
