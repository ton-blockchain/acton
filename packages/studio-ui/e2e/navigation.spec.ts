// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import type {StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Navigation test",
  status: "running",
  lifecycle: "managed",
  rpcUrl: "/rpc",
  config: {
    kind: "actonSimulatedLocalnet",
    port: 5411,
    accounts: [],
    noMining: false,
    mineEmptyBlocks: false,
  },
  capabilities: ["contracts", "explorer"],
  endpoints: {},
  network: {
    id: "local",
    label: "Local",
    chainId: -3,
    testOnly: true,
    supportsActions: true,
  },
}

const network: StudioEnvironment = {
  ...environment,
  id: "mainnet",
  name: "Mainnet",
  lifecycle: "external",
  capabilities: [],
}

test.beforeEach(async ({page}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname

    await route.fulfill({
      json:
        path === "/api/v1/info"
          ? {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
          : path === "/api/v1/environments"
            ? [environment, network]
            : [],
    })
  })
})

test("keeps real networks collapsed until requested", async ({page}) => {
  await page.goto("/virtual-environments")

  const toggle = page.getByRole("button", {name: "Real networks", exact: true})
  const disclosure = page.locator("#studio-networks-navigation")
  await expect(toggle).toHaveAttribute("aria-expanded", "false")
  await expect(disclosure).toHaveAttribute("aria-hidden", "true")

  await toggle.click()
  await expect(toggle).toHaveAttribute("aria-expanded", "true")
  await expect(disclosure).toHaveAttribute("aria-hidden", "false")
  await expect(disclosure.getByRole("button", {name: /Mainnet/})).toBeVisible()
})

test("shows Favorites in the environment Explorer menu", async ({page}) => {
  await page.goto("/virtual-environments/environment-1/explorer")

  const navigation = page.getByRole("navigation", {name: "Navigation test navigation"})
  await navigation.getByRole("button", {name: "Favorites", exact: true}).click()

  await expect(page).toHaveURL(/\/virtual-environments\/environment-1\/explorer\/favorites$/)
})

test("shows only page-specific environment header actions", async ({page}) => {
  await page.goto("/virtual-environments/environment-1/explorer")

  await expect(page.getByRole("button", {name: "Copy RPC endpoint"})).toHaveCount(0)

  await page.goto("/virtual-environments/environment-1/contracts")

  await expect(page.getByRole("button", {name: "Add contract"})).toBeVisible()
})
