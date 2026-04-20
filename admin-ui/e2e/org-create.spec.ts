/**
 * Regression test for #86 — org create schema drift.
 *
 * The bug: CreateOrgDialog sent `{ name, unit_type }`, but the backend
 * expects `{ name, description?, companyId }` and validates the company.
 * Result: three consecutive 422s on submit.
 *
 * These tests pin the correct wire contract:
 *   1. When companies exist, submitting the form POSTs a body that
 *      contains `companyId` and `name`, does NOT contain `unit_type`.
 *   2. When no Company units exist, the Create button is disabled and
 *      the amber hint is visible.
 */
import { test, expect } from "./fixtures"

const COMPANY = {
  id: "unit-company-01",
  name: "Acme Holding",
  unitType: "Company",
  parentId: null,
}

test("Create Org dialog sends { name, companyId } and never unit_type", async ({
  page,
  login,
}) => {
  let capturedPost: { url: string; body: unknown } | null = null

  await login({
    mocks: [
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "GET",
        body: [COMPANY],
      },
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "POST",
        handler: async (route) => {
          capturedPost = {
            url: route.request().url(),
            body: route.request().postDataJSON(),
          }
          await route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({
              id: "unit-org-new",
              name: "New Org",
              unitType: "Organization",
              parentId: COMPANY.id,
            }),
          })
        },
      },
    ],
  })

  await page.goto("/admin/organizations")

  // Wait for the tree to hydrate (the Company node renders its name).
  await expect(page.getByText("Acme Holding")).toBeVisible({ timeout: 10_000 })

  // Open the create dialog.
  await page.getByRole("button", { name: /create org/i }).click()

  const dialog = page.locator("role=dialog")
    .or(page.locator("form").filter({ has: page.getByLabel("Name") }))

  // The parent company select should be populated and defaulted.
  const companySelect = page.getByLabel(/parent company/i)
  await expect(companySelect).toBeVisible()
  await expect(companySelect).toHaveValue(COMPANY.id)

  // Fill the form and submit.
  await page.getByLabel("Name").fill("New Org")
  await page.getByLabel(/description/i).fill("Created by e2e test")
  await page.getByRole("button", { name: /^create$/i }).click()

  // Wait until the handler captured the POST.
  await expect.poll(() => capturedPost, { timeout: 5_000 }).not.toBeNull()

  // HARD contract assertions.
  const body = (capturedPost as unknown as { body: Record<string, unknown> }).body
  expect(body).toMatchObject({
    name: "New Org",
    companyId: COMPANY.id,
    description: "Created by e2e test",
  })
  expect(body).not.toHaveProperty("unit_type")
  expect(body).not.toHaveProperty("unitType")
  // Ensure we did not send either bogus parent field.
  expect(body).not.toHaveProperty("parentId")

  // Dialog should close on success.
  await expect(dialog).toBeHidden({ timeout: 5_000 })
})

test("Create Org dialog blocks submit when no Company units exist", async ({
  page,
  login,
}) => {
  await login({
    mocks: [
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "GET",
        body: [], // empty tree — no Company
      },
    ],
  })

  await page.goto("/admin/organizations")
  await page.getByRole("button", { name: /create org/i }).click()

  // Button is disabled.
  const createBtn = page.getByRole("button", { name: /^create$/i })
  await expect(createBtn).toBeDisabled()

  // The amber hint text is shown.
  await expect(page.getByText(/must be attached to a company/i)).toBeVisible()
})
