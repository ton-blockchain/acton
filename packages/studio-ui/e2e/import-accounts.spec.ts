// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import type {AdminOperation, ImportAccountsRequest, StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Import target",
  status: "running",
  lifecycle: "managed",
  rpcUrl: "/rpc",
  config: {
    kind: "fullTonNetwork",
    apiV2Port: 18_002,
    apiV3Port: 18_003,
    adminPort: 18_001,
    configPort: 18_000,
    observabilityPort: 18_007,
    importedAccounts: [],
    nodes: [],
  },
  capabilities: ["explorer", "contracts"],
  endpoints: {apiV2: "/rpc/api/v2", apiV3: "/rpc/api/v3"},
  network: {id: "local", label: "Local", chainId: -3, testOnly: true, supportsActions: false},
}
const source: StudioEnvironment = {
  ...environment,
  id: "mainnet",
  name: "Mainnet",
  lifecycle: "external",
  capabilities: [],
}
const address = `0:${"45".repeat(32)}`

for (const lostResponse of [false, true]) {
  test(`header import preserves progress through pause and reload${lostResponse ? " after a lost response" : ""}`, async ({
    page,
  }) => {
    const submitted: ImportAccountsRequest[] = []
    let operation: AdminOperation | null = null
    let status: StudioEnvironment["status"] = "running"
    let environmentPolls = 0
    await page.route("**/api/v1/**", async route => {
      const url = new URL(route.request().url())
      let body: unknown = []
      if (url.pathname.endsWith("/info")) body = {protocolVersion: 1, serverVersion: "test"}
      if (url.pathname === "/api/v1/environments") {
        environmentPolls += 1
        body = [{...environment, status}, source]
      }
      if (url.pathname.endsWith("/imports")) {
        submitted.push(route.request().postDataJSON())
        if (lostResponse && submitted.length === 1) {
          await route.abort()
          return
        }
        operation = {
          id: submitted[0].id,
          phase: "installing",
          startedAt: new Date().toISOString(),
          finishedAt: null,
          error: null,
          blockSeqno: null,
        }
        status = "starting"
        body = operation
      }
      if (url.pathname.endsWith("/admin")) {
        if (submitted.length > 0) expect(url.searchParams.get("id")).toBe(submitted[0].id)
        body = operation
      }
      await route.fulfill({json: body})
    })
    await page.goto("/virtual-environments/environment-1/dashboard")
    await page
      .getByLabel("State actions")
      .getByRole("button", {name: "Import accounts", exact: true})
      .click()
    const dialog = page.getByRole("dialog", {name: "Import accounts"})
    await dialog.getByRole("button", {name: "Add account"}).click()
    await expect(dialog.getByLabel("Source network for account 1").locator("option")).toHaveText([
      "Mainnet",
    ])
    await dialog.getByLabel("Account 1 contract name").fill("Imported contract")
    await dialog.getByLabel("Account 1 address").fill(address)
    await dialog.getByRole("button", {name: "Import accounts", exact: true}).click()
    if (lostResponse) {
      await expect(dialog.getByLabel("Account 1 contract name")).toBeDisabled()
      await page.reload()
      await expect(dialog).toBeVisible()
      await dialog.getByRole("button", {name: "Retry same import"}).click()
      expect(submitted[1]).toEqual(submitted[0])
    }
    await expect(dialog.getByRole("status")).toHaveText("Installing hardfork")
    const pollsBefore = environmentPolls
    await expect.poll(() => environmentPolls).toBeGreaterThan(pollsBefore)
    await expect(dialog).toBeVisible()
    await expect(dialog.getByLabel("Account 1 address")).toBeDisabled()
    await dialog.getByRole("button", {name: "Close", exact: true}).click()
    await page.reload()
    await expect(dialog).toBeVisible()
    await expect(dialog.getByRole("status")).toHaveText("Installing hardfork")
    status = "running"
    operation = {
      ...(operation as unknown as AdminOperation),
      phase: "completed",
      finishedAt: new Date().toISOString(),
      blockSeqno: 123,
    }
    await expect(dialog.getByRole("status")).toHaveText("Accounts imported")
    await expect(dialog.getByRole("link", {name: "View contracts"})).toHaveAttribute(
      "href",
      "/virtual-environments/environment-1/contracts",
    )
    expect(submitted[0].accounts).toEqual([
      {sourceEnvironmentId: "mainnet", address, name: "Imported contract"},
    ])
    expect(
      await page.evaluate(() =>
        localStorage.getItem("actonStudioEnvironment:environment-1:accountImport"),
      ),
    ).toBeNull()
  })
}

test("rejected source import reports a toast and allows correcting the shared form", async ({
  page,
}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith("/imports")) {
      await route.fulfill({
        status: 400,
        json: {error: {message: "Account does not exist in Mainnet"}},
      })
      return
    }
    await route.fulfill({
      json: path.endsWith("/info")
        ? {protocolVersion: 1, serverVersion: "test"}
        : path.endsWith("/environments")
          ? [environment, source]
          : path.endsWith("/admin")
            ? null
            : [],
    })
  })
  await page.goto("/virtual-environments/environment-1/dashboard")
  await page.getByRole("button", {name: "Import accounts", exact: true}).click()
  const dialog = page.getByRole("dialog", {name: "Import accounts"})
  await dialog.getByRole("button", {name: "Add account"}).click()
  await dialog.getByLabel("Account 1 address").fill(address)
  await dialog.getByRole("button", {name: "Import accounts", exact: true}).click()
  await expect(page.getByRole("region", {name: "Notifications"})).toContainText(
    "Account does not exist in Mainnet",
  )
  await expect(dialog.getByLabel("Account 1 address")).toBeEnabled()
  await expect(dialog).not.toContainText("Account does not exist in Mainnet")
})
