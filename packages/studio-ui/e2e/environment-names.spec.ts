// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import type {StudioEnvironment} from "../src/studioApi"

let environments: StudioEnvironment[]

test.beforeEach(async ({page}) => {
  environments = [
    simulatedEnvironment("environment-1", "Simulator 1"),
    simulatedEnvironment("environment-2", "Simulator 3"),
    simulatedEnvironment("environment-3", "Localnet 1"),
  ]

  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname

    await route.fulfill({
      json:
        path === "/api/v1/info"
          ? {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
          : path === "/api/v1/environments"
            ? environments
            : [],
    })
  })
})

test("suggests an unused environment name and releases deleted names", async ({page}) => {
  await page.goto("/virtual-environments")
  await page.getByRole("button", {name: "Create environment"}).click()

  await expect(page.getByLabel("Name")).toHaveValue("Simulator 2")

  await page.getByLabel("Environment type").selectOption("fullTonNetwork")
  await expect(page.getByLabel("Name")).toHaveValue("Localnet 2")

  await page.getByLabel("Name").fill("Custom environment")
  await page.getByLabel("Environment type").selectOption("actonSimulatedLocalnet")
  await expect(page.getByLabel("Name")).toHaveValue("Custom environment")

  await page.getByRole("button", {name: "Cancel"}).click()
  environments.splice(0, 1)
  await page.reload()
  await page.getByRole("button", {name: "Create environment"}).click()

  await expect(page.getByLabel("Name")).toHaveValue("Simulator 1")
})

function simulatedEnvironment(id: string, name: string): StudioEnvironment {
  return {
    id,
    name,
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
    capabilities: ["explorer"],
    endpoints: {},
    network: {
      id: "local",
      label: "Local",
      chainId: -3,
      testOnly: true,
      supportsActions: true,
    },
  }
}
