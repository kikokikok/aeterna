## Why

The admin UI static asset route in `server-runtime` serves `index.html` as an SPA fallback for unmatched `/admin/*` paths. Today the fallback response is emitted with HTTP status `404 Not Found` because it is plumbed through the `ServeDir` fallback handler. Browsers render the HTML correctly, but:

1. **Monitoring false-alarms**: every deep link shows up as a 404 in access logs and CloudFront/ALB metrics, inflating the error rate and making real 404s (broken external links) invisible.
2. **Crawlers and link checkers**: tools like Lighthouse, Sentry's `rrweb`, and internal link-checkers flag every SPA route as broken.
3. **CDN caching**: some CDNs refuse to cache 4xx responses by default, so the SPA shell is re-fetched on every deep-link, negating the benefit of a built bundle.

This is a small, mechanical fix: SPA fallbacks MUST return `200 OK` with the HTML body. The `ServeDir` configuration needs a dedicated fallback that sets the status explicitly.

## What Changes

- Change the `/admin/*` SPA fallback to return `200 OK` instead of `404 Not Found`.
- Emit the same `index.html` body (unchanged).
- Real non-admin 404s (paths that don't match any handler at all) continue to return `404 Not Found` — the fix is scoped to the `/admin/*` prefix.
- Add a smoke test that asserts `curl -I /admin/nonexistent-route` returns `200`.
- Update `README.md` and `docs/deployment.md` to note the behavior for operators configuring monitoring dashboards.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `server-runtime`: the SPA fallback for the `/admin/*` route group returns `200 OK` with the admin UI `index.html` body instead of `404 Not Found`.

## Impact

- **Affected code**: `cli/src/server/router.rs` (replace `ServeDir` default fallback with a custom handler that responds with 200 + index.html bytes), possibly a small helper in `cli/src/server/admin_ui.rs`.
- **Affected APIs**: none — this is a status-code correction on the admin UI static asset response only.
- **Backward compatibility**: first-party clients that matched the browser's URL bar on 404 (there shouldn't be any) see the old path succeed. External monitoring dashboards should reclassify `/admin/*` responses if they previously filtered on 404.
