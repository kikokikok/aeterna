/**
 * E2E coverage for the generic Create-Unit dialog.
 *
 * History:
 *   - Originally written for issue #86 (org-create schema drift). The first
 *     version of this test pinned the OLD wire contract:
 *         { name, description?, companyId } and explicitly NOT unit_type
 *     because the backend hardcoded UnitType::Organization.
 *   - In v1.5.x the backend POST /api/v1/org was generalised to accept any
 *     UnitType (commit e44bbd21). This test was rewritten to pin the new
 *     contract: { name, description?, unitType, parentId? }.
 *   - The deprecated `companyId` alias is still accepted by the server for
 *     CLI back-compat, but the admin-ui dialog deliberately does NOT send
 *     it. These tests guard that.
 *
 * What is pinned here:
 *   1. Creating a Tenant sends { unitType: "Tenant" } and NO parentId.
 *   2. Creating an Organization sends { unitType, parentId } where parent
 *      is a tenant-root organization.
 *   3. Creating a Team sends parentId pointing at an Organization; the
 *      parent picker is filtered to Organizations only.
 *   4. Creating a Project sends parentId pointing at a Team; parent picker
 *      is filtered to Teams only.
 *   5. The dialog NEVER sends `unit_type` (snake_case) or `companyId`.
 *   6. When no eligible parent exists for the chosen type, Create is
 *      disabled and an amber hint is shown.
 */
import { test, expect } from "./fixtures"

const COMPANY = {
  id: "unit-root-org-01",
  name: "Acme Root Org",
  unitType: "Tenant",
  parentId: null,
}
const ORG = {
  id: "unit-org-01",
  name: "Acme Treasury",
  unitType: "Organization",
  parentId: COMPANY.id,
}
const TEAM = {
  id: "unit-team-01",
  name: "Cash Forecasting Team",
  unitType: "Team",
  parentId: ORG.id,
}

test("Create Organization sends unitType and NO parentId", async ({ page, login }) => {
  let capturedPost: { body: Record<string, unknown> } | null = null

  await login({
    mocks: [
      { urlPattern: /\/api\/v1\/org$/, method: "GET", body: [] },
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "POST",
        handler: async (route) => {
          capturedPost = { body: route.request().postDataJSON() as Record<string, unknown> }
          await route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({
              id: "unit-root-org-new",
              name: "New Root Org",
              unitType: "Tenant",
              parentId: null,
            }),
          })
        },
      },
    ],
  })

  await page.goto("/admin/organizations")
  await page.getByRole("button", { name: /create unit/i }).click()

  await page.getByLabel(/^type$/i).selectOption("Tenant")
  // The parent select should not be rendered for the tenant root.
  await expect(page.getByLabel(/parent /i)).toHaveCount(0)

  await page.getByLabel(/^name$/i).fill("New Root Org")
  await page.getByRole("button", { name: /^create$/i }).click()

  await expect.poll(() => capturedPost, { timeout: 5_000 }).not.toBeNull()
  const body = (capturedPost as unknown as { body: Record<string, unknown> }).body
  expect(body).toMatchObject({ name: "New Root Org", unitType: "Tenant" })
  expect(body).not.toHaveProperty("parentId")
  expect(body).not.toHaveProperty("companyId")
  expect(body).not.toHaveProperty("unit_type")
})

test("Create Organization sends unitType + parentId pointing at the tenant root", async ({
  page,
  login,
}) => {
  let capturedPost: { body: Record<string, unknown> } | null = null

  await login({
    mocks: [
      { urlPattern: /\/api\/v1\/org$/, method: "GET", body: [COMPANY] },
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "POST",
        handler: async (route) => {
          capturedPost = { body: route.request().postDataJSON() as Record<string, unknown> }
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
  await expect(page.getByText("Acme Root Org")).toBeVisible({ timeout: 10_000 })
  await page.getByRole("button", { name: /create unit/i }).click()

  // Default selection should be Organization (the dialog's defaultUnitType).
  await expect(page.getByLabel(/^type$/i)).toHaveValue("Organization")

  // Parent picker is labelled "Parent Organization" and is populated.
  const parentSelect = page.getByLabel(/parent organization/i)
  await expect(parentSelect).toBeVisible()
  await expect(parentSelect).toHaveValue(COMPANY.id)

  await page.getByLabel(/^name$/i).fill("New Org")
  await page.getByLabel(/description/i).fill("Created by e2e test")
  await page.getByRole("button", { name: /^create$/i }).click()

  await expect.poll(() => capturedPost, { timeout: 5_000 }).not.toBeNull()
  const body = (capturedPost as unknown as { body: Record<string, unknown> }).body
  expect(body).toMatchObject({
    name: "New Org",
    description: "Created by e2e test",
    unitType: "Organization",
    parentId: COMPANY.id,
  })
  expect(body).not.toHaveProperty("companyId")
  expect(body).not.toHaveProperty("unit_type")
})

test("Create Team filters parent picker to Organizations only", async ({ page, login }) => {
  let capturedPost: { body: Record<string, unknown> } | null = null

  await login({
    mocks: [
      { urlPattern: /\/api\/v1\/org$/, method: "GET", body: [COMPANY, ORG] },
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "POST",
        handler: async (route) => {
          capturedPost = { body: route.request().postDataJSON() as Record<string, unknown> }
          await route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({
              id: "unit-team-new",
              name: "New Team",
              unitType: "Team",
              parentId: ORG.id,
            }),
          })
        },
      },
    ],
  })

  await page.goto("/admin/organizations")
  await page.getByRole("button", { name: /create unit/i }).click()
  await page.getByLabel(/^type$/i).selectOption("Team")

  const parentSelect = page.getByLabel(/parent organization/i)
  await expect(parentSelect).toBeVisible()
  // Only Organizations should be in the options pool — Acme Root Org (Tenant)
  // must NOT be selectable.
  const optionLabels = await parentSelect.evaluate((el) =>
    Array.from(el.querySelectorAll("option")).map((o) => o.textContent ?? ""),
  )
  expect(optionLabels).toContain("Acme Treasury")
  expect(optionLabels).not.toContain("Acme Root Org")
  await expect(parentSelect).toHaveValue(ORG.id)

  await page.getByLabel(/^name$/i).fill("New Team")
  await page.getByRole("button", { name: /^create$/i }).click()

  await expect.poll(() => capturedPost, { timeout: 5_000 }).not.toBeNull()
  const body = (capturedPost as unknown as { body: Record<string, unknown> }).body
  expect(body).toMatchObject({
    name: "New Team",
    unitType: "Team",
    parentId: ORG.id,
  })
})

test("Create Project filters parent picker to Teams only", async ({ page, login }) => {
  let capturedPost: { body: Record<string, unknown> } | null = null

  await login({
    mocks: [
      { urlPattern: /\/api\/v1\/org$/, method: "GET", body: [COMPANY, ORG, TEAM] },
      {
        urlPattern: /\/api\/v1\/org$/,
        method: "POST",
        handler: async (route) => {
          capturedPost = { body: route.request().postDataJSON() as Record<string, unknown> }
          await route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({
              id: "unit-project-new",
              name: "New Project",
              unitType: "Project",
              parentId: TEAM.id,
            }),
          })
        },
      },
    ],
  })

  await page.goto("/admin/organizations")
  await page.getByRole("button", { name: /create unit/i }).click()
  await page.getByLabel(/^type$/i).selectOption("Project")

  const parentSelect = page.getByLabel(/parent team/i)
  await expect(parentSelect).toBeVisible()
  const optionLabels = await parentSelect.evaluate((el) =>
    Array.from(el.querySelectorAll("option")).map((o) => o.textContent ?? ""),
  )
  expect(optionLabels).toContain("Cash Forecasting Team")
  expect(optionLabels).not.toContain("Acme Treasury")
  expect(optionLabels).not.toContain("Acme Root Org")

  await page.getByLabel(/^name$/i).fill("New Project")
  await page.getByRole("button", { name: /^create$/i }).click()

  await expect.poll(() => capturedPost, { timeout: 5_000 }).not.toBeNull()
  const body = (capturedPost as unknown as { body: Record<string, unknown> }).body
  expect(body).toMatchObject({
    name: "New Project",
    unitType: "Project",
    parentId: TEAM.id,
  })
})

test("Create Org is blocked when no Tenant units exist", async ({ page, login }) => {
  await login({
    mocks: [{ urlPattern: /\/api\/v1\/org$/, method: "GET", body: [] }],
  })

  await page.goto("/admin/organizations")
  await page.getByRole("button", { name: /create unit/i }).click()
  // Default unit type is Organization, which requires a Tenant parent.
  await expect(page.getByLabel(/^type$/i)).toHaveValue("Organization")
  await expect(page.getByRole("button", { name: /^create$/i })).toBeDisabled()
  // The amber-coloured hint (uniquely identified by "Create an Organization unit")
  // is shown when no eligible parent exists. Match the longer unique phrase
  // to avoid colliding with the always-visible helper text under the type
  // select that also says "must be attached to an Organization".
  await expect(page.getByText(/Create an Organization unit for this tenant first/i)).toBeVisible()
})
