# admin-ui end-to-end tests

Hermetic Playwright regression suite for the Aeterna admin UI.

## Philosophy

- **No backend required.** Every test intercepts HTTP with `page.route()`
  and returns deterministic fixtures. This keeps the suite fast (~seconds)
  and runnable in any CI without infra dependencies.
- **One bug → one spec.** Each file pins the behavior of a previously shipped
  regression so it can't silently come back.
- **Contract-level assertions.** Tests assert on the shape/URL of outbound
  requests and the text/roles users see — not on internal component state.

## Running

```bash
cd admin-ui
npm ci
npm run test:e2e:install   # one-time: download chromium
npm run test:e2e           # headless
npm run test:e2e:ui        # interactive UI mode
npm run test:e2e -- --debug
```

The `webServer` in `playwright.config.ts` boots `npm run dev` on port 5173
automatically. Set `CI=1` to enable retries and GitHub-Actions reporting.

## Regression index

| Spec                      | Issue / PR | Bug                                                                   |
| ------------------------- | ---------- | --------------------------------------------------------------------- |
| `tenant-detail.spec.ts`   | #85 / PR#85| TenantDetailPage fetched `/tenants/undefined/*` due to envelope leak. |
| `org-create.spec.ts`      | #86 / PR#88| Create Org dialog sent `{ unit_type }` instead of `{ companyId }`.    |
| `memory-search.spec.ts`   | #87 / PR#89| Memory handlers 502'd on internal errors; UI contract-pinned here.    |

## Adding a new spec

1. Create `<feature>.spec.ts` in this directory.
2. `import { test, expect } from "./fixtures"`.
3. Call `await login({ mocks: [...] })` before `page.goto()`.
4. Prefer `getByRole` / `getByLabel` / `getByText` over brittle CSS locators.
5. When asserting on an outbound request, prefer `handler` with
   `route.request().postDataJSON()` over waiting for a global side-effect.

## CI

The GitHub Actions workflow `.github/workflows/admin-ui-e2e.yml` runs this
suite on every PR touching `admin-ui/**`.
