# Webhooker — Review TODO

## 🟠 HIGH

- [x] **No action queue — actions run synchronously** — Postgres-backed queue with dedicated worker pool.
- [x] **Webhook SSRF** — URL validation with configurable strict/relaxed mode and CIDR allowlist.
- [x] **Template injection** — HTML-escaped interpolation for HTML email bodies.
- [x] **No body size limit enforced** — `RequestBodyLimitLayer` applied globally.

## 🟡 MEDIUM

- [x] **Registration race condition** — Advisory lock (`pg_advisory_xact_lock`) in transaction prevents concurrent bootstrap registrations.
- [x] **Export query not tenant-scoped** — Added tenant_id join to list_for_export query.
- [x] **Crypto key uses raw SHA-256** — Replaced with HKDF-SHA256 key derivation.
- [x] **Rate limit cleanup never called** — Periodic cleanup task every 5 min, evicts entries older than 30 min.
- [x] **Cookie security flags missing** — Server-side HttpOnly/Secure/SameSite=Lax cookies, removed JS cookie handling.
- [x] **System admin can delete own tenant** — Guard prevents deleting own tenant.
- [x] **Sort column injection fragile** — Replaced with `SortColumn`/`SortOrder` enums that map to static SQL strings.
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
