import {expect, test, type Page, type Route} from "@playwright/test"

import {config, fileContents, reports, traces} from "./fixture-data"

async function fulfillJson(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  })
}

async function mockTestUiApi(page: Page): Promise<void> {
  await page.route("**/api/config", route => fulfillJson(route, config))
  await page.route("**/api/reports", route => fulfillJson(route, reports))
  await page.route("**/api/trace/*", async route => {
    const url = new URL(route.request().url())
    const traceName = decodeURIComponent(url.pathname.slice("/api/trace/".length))
    const trace = traces[traceName]
    if (trace === undefined) {
      await fulfillJson(route, {error: "Trace not found"}, 404)
      return
    }
    await fulfillJson(route, trace)
  })
  await page.route("**/api/contract/*", route => fulfillJson(route, {error: "Contract not found"}, 404))
  await page.route(/\/api\/file\?path=.*/, async route => {
    const filePath = new URL(route.request().url()).searchParams.get("path")
    const file = filePath === null ? undefined : fileContents[filePath]
    await route.fulfill({
      status: file === undefined ? 404 : 200,
      contentType: "text/plain; charset=utf-8",
      body: file ?? "File not found",
    })
  })
}

test("integration smoke loads reports over the local fixture API", async ({page}) => {
  await page.goto("/")

  await expect(page.getByText("Test UI")).toBeVisible()
  await expect(page.getByRole("button", {name: "counter increments"})).toBeVisible()
  await expect(page.getByText("Status")).toBeVisible()
  await expect(page.getByText(/^Passed$/).last()).toBeVisible()
})

test.describe("mocked regression flows", () => {
  test.beforeEach(async ({page}) => {
    await mockTestUiApi(page)
  })

  test("filters the test list by search and status", async ({page}) => {
    await page.goto("/")

    await page.getByPlaceholder("Filter tests...").fill("invalid")
    await expect(page.getByRole("button", {name: "rejects invalid opcode"})).toBeVisible()
    await expect(page.getByRole("button", {name: "counter increments"})).toHaveCount(0)

    await page.locator('[title="Show Failed tests"]').click()
    await expect(page.getByRole("button", {name: "rejects invalid opcode"})).toHaveCount(0)
  })

  test("renders failed trace details across tabs", async ({page}) => {
    await page.goto("/")

    await page.getByRole("button", {name: "rejects invalid opcode"}).click()

    await expect(page.getByText("Error Message")).toBeVisible()
    await expect(page.getByText("Unexpected exit code: got 35, expected 0")).toBeVisible()
    await expect(page.getByText("tests/counter.spec.tolk")).toBeVisible()

    await page.getByRole("button", {name: "Transactions"}).click()
    await expect(page.getByText("External message was not accepted").first()).toBeVisible()
    await expect(page.getByText("VM Exit Code")).toBeVisible()

    await page.getByRole("button", {name: "Logs"}).click()
    await expect(page.getByText("vm rejected external message")).toBeVisible()
  })

  test("persists theme and collapsed sidebar across reload", async ({page}) => {
    await page.goto("/")

    await page.locator('[title="Switch to dark theme"]').click()
    await expect(page.locator("html")).toHaveClass(/dark-theme/)

    await page.locator('[title="Collapse sidebar"]').click()
    await expect(page.locator('[title="Expand sidebar"]')).toBeVisible()

    await page.reload()

    await expect(page.locator("html")).toHaveClass(/dark-theme/)
    await expect(page.locator('[title="Expand sidebar"]')).toBeVisible()
  })
})
