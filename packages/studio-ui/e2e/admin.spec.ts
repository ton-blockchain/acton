// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import type {AdminOperation, AdminRequest, StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Hardfork test",
  status: "running",
  lifecycle: "managed",
  rpcUrl: "/rpc",
  config: {
    kind: "fullTonNetwork",
    apiV2Port: 18_002,
    apiV3Port: 8081,
    adminPort: 18_001,
    configPort: 18_000,
    observabilityPort: 18_007,
    nodes: [],
    importedAccounts: [],
  },
  capabilities: ["explorer", "snapshots"],
  endpoints: {},
  network: {id: "local", label: "Local", chainId: -3, testOnly: true, supportsActions: false},
}

test("admin form submits nanotons and tracks a detached operation across reload", async ({
  page,
}) => {
  let operation: AdminOperation | null = null
  let submitted: AdminRequest | undefined
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    let body: unknown = []
    if (path === "/api/v1/info")
      body = {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
    else if (path === "/api/v1/environments") body = [environment]
    else if (path.endsWith("/admin")) {
      if (route.request().method() === "POST") {
        submitted = route.request().postDataJSON() as AdminRequest
        operation = {
          id: submitted.id,
          phase: "installing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        }
      }
      body = operation
    }
    await route.fulfill({json: body})
  })
  await page.goto("/virtual-environments/environment-1/admin")
  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("New balance (TON)").fill("12.5")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  expect(submitted?.kind).toBe("accounts")
  if (submitted?.kind === "accounts")
    expect(submitted.edits[0]).toEqual({
      address: `0:${"11".repeat(32)}`,
      type: "balance",
      balance: "12500000000",
    })
  await expect(page.getByText("Installing hardfork", {exact: true})).toBeVisible()
  await expect(page.getByLabel("Account address")).toBeDisabled()
  await page.reload()
  await expect(page.getByText("Installing hardfork", {exact: true})).toBeVisible()
  operation = {
    id: submitted?.id ?? "",
    startedAt: new Date().toISOString(),
    error: null,
    phase: "completed",
    finishedAt: new Date().toISOString(),
    blockSeqno: 1234,
  }
  await expect(page.getByText("Changes applied", {exact: true})).toBeVisible()
  await page.screenshot({path: "/tmp/acton-hardfork-review/admin-ui.png", fullPage: true})
})

test("ambiguous HTTP failure retries the exact same request", async ({page}) => {
  const requests: AdminRequest[] = []
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith("/admin") && route.request().method() === "POST") {
      requests.push(route.request().postDataJSON())
      if (requests.length === 1) {
        await route.abort()
        return
      }
      await route.fulfill({
        json: {
          id: requests[0].id,
          phase: "preparing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        },
      })
      return
    }
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [environment]
          : path.endsWith("/admin")
            ? null
            : [],
    })
  })
  await page.goto("/virtual-environments/environment-1/admin")
  await page.getByLabel("Account address").fill(`0:${"11".repeat(32)}`)
  await page.getByLabel("New balance (TON)").fill("7")
  await page.getByRole("button", {name: "Apply changes", exact: true}).click()
  await page.getByRole("button", {name: "Retry same operation"}).click()
  expect(requests).toHaveLength(2)
  expect(requests[0]).toEqual(requests[1])
})
