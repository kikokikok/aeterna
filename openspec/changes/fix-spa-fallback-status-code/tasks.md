## 1. Implementation

- [ ] 1.1 Replace the `ServeDir::not_found_service` / `fallback` call in `cli/src/server/router.rs` (admin UI route group) with a custom `axum::routing::any_service` fallback that reads `index.html` once at startup and returns it with `StatusCode::OK` and `Content-Type: text/html; charset=utf-8`.
- [ ] 1.2 Cache the `index.html` body in an `Arc<Bytes>` at router build time (no disk read per request).
- [ ] 1.3 Keep the real static files served by `ServeDir` (hits for `main.js`, `styles.css`, favicon, etc.) unchanged and ensure they still short-circuit before the fallback.
- [ ] 1.4 Ensure non-`/admin/*` paths continue to receive the default `404` fallback.

## 2. Tests

- [ ] 2.1 Integration test: `GET /admin/nonexistent/deep/path` returns `200` with `Content-Type: text/html` and body starting with `<!DOCTYPE html>`.
- [ ] 2.2 Integration test: `GET /admin/main.js` (existing static file) returns `200` with `Content-Type: application/javascript` and the actual bundle content (not index.html).
- [ ] 2.3 Integration test: `GET /nonexistent-top-level-path` returns `404 Not Found` (confirming the fix is scoped).
- [ ] 2.4 Integration test: `AETERNA_ADMIN_UI_PATH` unset → `GET /admin/whatever` returns `404` (no admin UI → no SPA fallback).

## 3. Documentation and ops

- [ ] 3.1 Update `docs/deployment.md` (or create if missing) noting the `/admin/*` route returns `200` for SPA paths.
- [ ] 3.2 Add a note to `README.md` monitoring section to exclude `/admin/*` from 404 alerting dashboards where applicable.
