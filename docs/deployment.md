# Deployment notes

## Admin UI routing (`/admin/*`)

The Aeterna server optionally serves a compiled React admin UI at `/admin`
when `AETERNA_ADMIN_UI_PATH` points to a directory containing `index.html`
(default `./admin-ui/dist`).

Routing policy:

| Request | Response |
|---|---|
| `GET /admin/main.js` (or any real file in `dist/`) | `200` with the file's actual content type |
| `GET /admin/nonexistent/deep/path` (no matching static file) | **`200 text/html; charset=utf-8`** with the cached `index.html` shell |
| `GET /nonexistent-top-level-path` (outside `/admin`) | `404` JSON |

Serving the SPA shell with `200` (rather than the historic `404`) mirrors
the behavior of every mainstream static host (Netlify, Cloudflare Pages,
S3 + CloudFront "SPA mode"). The React Router inside the bundle is
authoritative for "page not found" rendering — the server has no way to
know whether `/admin/things/42` maps to a real client-side route without
executing the bundle.

### Monitoring

Dashboards that alert on `http_requests_total{status="404"}` should
**exclude `/admin/*` paths**. Prior to this change, missing client-side
routes (e.g. after a user pasted a stale URL into the browser) showed up
as server 404s and inflated error-rate SLOs. Those requests now render as
`200` with the SPA shell as intended.

### Cache semantics

The SPA shell is served with `Cache-Control: no-cache` so clients always
revalidate and pick up updated hashed asset URLs. The hashed assets
referenced from inside the shell (`main.<hash>.js`, `styles.<hash>.css`)
are served by `ServeDir` with tower-http's default strong caching.

### Opting out

Unset `AETERNA_ADMIN_UI_PATH`, or point it at a directory that does not
contain an `index.html`. The `/admin` route is then not registered and
all `/admin/*` requests fall through to the top-level `404`.
