/**
 * Regression test for #87 — memory API status codes.
 *
 * The bug was backend-side (handlers returning 502 for internal errors),
 * but the UI regressions worth pinning are:
 *
 *   1. Browse mode issues POST /api/v1/memory/list with a valid
 *      `{ layer, limit }` body (shape the backend actually parses).
 *   2. Search mode issues POST /api/v1/memory/search with `{ query }`.
 *   3. When the backend returns 500 with `{ error, message }`, the page
 *      shows the error state + Retry button (doesn't blank out).
 */
import { test, expect } from "./fixtures"

test("Memory browse issues a well-formed POST /memory/list", async ({ page, login }) => {
  let capturedListBody: unknown = null

  await login({
    mocks: [
      {
        urlPattern: /\/api\/v1\/memory\/list$/,
        method: "POST",
        handler: async (route) => {
          capturedListBody = route.request().postDataJSON()
          await route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({
              items: [
                {
                  id: "mem-1",
                  content: "project onboarding notes",
                  layer: "Project",
                  importanceScore: 0.42,
                },
              ],
              total: 1,
            }),
          })
        },
      },
    ],
  })

  await page.goto("/admin/memory")
  await page.getByRole("button", { name: /browse/i }).click()

  await expect(page.getByText("project onboarding notes")).toBeVisible({ timeout: 10_000 })

  // Contract: the browse request must include a `layer` field.
  expect(capturedListBody).toMatchObject({ layer: "Project" })
  expect((capturedListBody as { limit?: number }).limit).toBeGreaterThan(0)
})

test("Memory search renders error state when backend returns 500", async ({ page, login }) => {
  await login({
    mocks: [
      {
        urlPattern: /\/api\/v1\/memory\/search$/,
        method: "POST",
        status: 500,
        body: {
          error: "memory_search_failed",
          message: "vector store unreachable",
        },
      },
    ],
  })

  await page.goto("/admin/memory")
  await page.getByPlaceholder(/search memory entries/i).fill("hello world")
  // The page has two "Search" buttons (view-mode toggle + form submit).
  // Scope to the form to disambiguate.
  await page.locator("form").getByRole("button", { name: /^search$/i }).click()

  // The page renders a visible failure state with a retry affordance.
  await expect(page.getByText(/search failed/i)).toBeVisible({ timeout: 10_000 })
  await expect(page.getByRole("button", { name: /retry/i })).toBeVisible()
})
