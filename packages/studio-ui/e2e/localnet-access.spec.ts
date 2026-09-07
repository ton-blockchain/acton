// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import type {StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Docker recovery",
  status: "failed",
  lifecycle: "managed",
  rpcUrl: "/api/v1/environments/environment-1/rpc",
  config: {
    kind: "fullTonNetwork",
    apiV2Port: 18_002,
    apiV3Port: 18_003,
    adminPort: 18_001,
    configPort: 18_000,
    observabilityPort: 18_004,
    nodes: [],
    importedAccounts: [],
  },
  capabilities: ["explorer", "controlApi", "snapshots", "wallets"],
  endpoints: {},
  network: {id: "local", label: "Local", chainId: -3, testOnly: true, supportsActions: false},
}

test("Docker recovery stays on the startup page and retries without reloading Studio", async ({
  page,
}) => {
  const diagnostics = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock"
  let state: StudioEnvironment = {
    ...environment,
    error: `Docker is not running\nStart Docker Desktop or your Docker Engine service, wait until it is ready, then retry\n\n${diagnostics}\nFull log: /tmp/network/startup.log`,
  }
  let retries = 0
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    let body: unknown = []
    if (path === "/api/v1/info") {
      body = {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
    } else if (path.endsWith("/restart")) {
      retries += 1
      state = {...environment, status: "running"}
      body = state
    } else if (path === "/api/v1/environments") {
      body = [state]
    }
    await route.fulfill({json: body})
  })

  await page.goto("/virtual-environments/environment-1/dashboard")
  await expect(page.getByText("Docker is not running", {exact: true})).toBeVisible()
  await expect(
    page.getByText(
      "Start Docker Desktop or your Docker Engine service, wait until it is ready, then retry",
      {exact: true},
    ),
  ).toBeVisible()
  await expect(page.getByText("Diagnostic details", {exact: true})).toBeVisible()
  await expect(page.getByText(diagnostics, {exact: false})).toBeVisible()
  await expect(page.getByRole("region", {name: "Notifications"})).not.toContainText(
    "Docker is not running",
  )
  await page.getByRole("button", {name: "Retry", exact: true}).click()
  await expect(page.getByText("Diagnostic details", {exact: true})).toHaveCount(0)
  expect(retries).toBe(1)
  await expect(page).toHaveURL(/environment-1\/dashboard$/)
})

test("full localnet ignores a stale browser token and never requests one after a 401", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("actonStudioEnvironment:environment-1:apiToken", "stale-browser-token")
  })
  const authorization: (string | undefined)[] = []
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.includes("/rpc/")) {
      authorization.push(route.request().headers().authorization)
      await route.fulfill({status: 401, json: {error: "Upstream rejected request"}})
      return
    }

    let body: unknown = []
    if (path === "/api/v1/info") {
      body = {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
    } else if (path === "/api/v1/environments") {
      body = [{...environment, status: "running"}]
    }
    await route.fulfill({json: body})
  })

  await page.goto("/virtual-environments/environment-1/dashboard")
  await expect.poll(() => authorization.length).toBeGreaterThan(0)
  expect(authorization.every(value => value === undefined)).toBe(true)
  await expect(page.getByRole("button", {name: /environment API token/i})).toHaveCount(0)
  await expect(page.getByRole("dialog", {name: /Localnet API token/})).toHaveCount(0)
})

test("simulated localnet keeps browser token access", async ({page}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    let body: unknown = []
    if (path === "/api/v1/info") {
      body = {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
    } else if (path === "/api/v1/environments") {
      body = [
        {
          ...environment,
          status: "running",
          config: {
            kind: "actonSimulatedLocalnet",
            port: 8080,
            accounts: [],
            noMining: false,
            mineEmptyBlocks: false,
          },
        },
      ]
    }
    await route.fulfill({json: body})
  })

  await page.goto("/virtual-environments/environment-1/dashboard")
  await page.getByRole("button", {name: "Set environment API token", exact: true}).click()
  await expect(page.getByRole("dialog", {name: "Localnet API token", exact: true})).toBeVisible()
  await expect(page.getByText(/ACTON_LOCALNET_AUTH_TOKEN/)).toBeVisible()
})
