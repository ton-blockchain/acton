// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"
import {
  CELL_INSPECTOR_FIXTURES,
  CELL_INSPECTOR_VERIFIED_SOURCE,
  TVM_CODE_HASH,
} from "../../ui-e2e/e2e/support/cellInspector"
import type {StudioEnvironment} from "../src/studioApi"

const environment: StudioEnvironment = {
  id: "environment-1",
  name: "Cell Inspector test",
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
  capabilities: ["explorer", "simulator"],
  endpoints: {},
  network: {
    id: "local",
    label: "Local",
    chainId: -3,
    testOnly: true,
    supportsActions: true,
  },
}

test("opens Cell Inspector verification in the verifier", async ({page}) => {
  await page.route("**/api/v1/**", async route => {
    const path = new URL(route.request().url()).pathname

    await route.fulfill({
      json:
        path === "/api/v1/info"
          ? {protocolVersion: 1, serverVersion: "test", workspace: {name: "test"}}
          : path === "/api/v1/environments"
            ? [environment]
            : [],
    })
  })

  await page.route("https://verifier-staging.ton.org/api/v1/verification/source?**", route =>
    route.fulfill({json: CELL_INSPECTOR_VERIFIED_SOURCE}),
  )

  await page.goto("/virtual-environments/environment-1/cell-inspector")
  await page.locator("#cell-inspector-input").fill(CELL_INSPECTOR_FIXTURES.tvmCodeHex)

  const verificationLink = page.getByRole("link", {name: "View verification", exact: true})
  await expect(verificationLink).toHaveAttribute(
    "href",
    `https://verifier-staging.ton.org/${TVM_CODE_HASH}`,
  )
  await expect(verificationLink).toHaveAttribute("target", "_blank")
})
