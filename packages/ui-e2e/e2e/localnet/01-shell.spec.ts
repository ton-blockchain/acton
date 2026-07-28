import {expect, test} from "@playwright/test"

import {expectVisualSnapshot, prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

test.describe("Localnet shell", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "localnet"})
    await page.goto("/explorer")
    await expect(page.getByRole("complementary", {name: "Main navigation"})).toBeVisible()
    await expect(page.getByRole("combobox", {name: "Explorer search"})).toBeVisible()
  })

  test("renders explorer inside dashboard navigation", async ({page}) => {
    await expect(page.getByRole("button", {name: "Collapse navigation"})).toBeVisible()
    await expect(page.getByRole("button", {name: "Set localnet API token"})).toBeVisible()

    const developerTools = page.getByRole("navigation", {name: "Developer tools"})
    await expect(developerTools.getByRole("link", {name: /Emulate/})).toHaveAttribute(
      "href",
      "/explorer/emulate",
    )
    await expect(developerTools.getByRole("link", {name: /Cell Inspector/})).toHaveAttribute(
      "href",
      "/explorer/cell",
    )
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    test("loc-shell-explorer-light", async ({page}) => {
      await expectVisualSnapshot(page, "loc-shell-explorer-light")
    })

    test("loc-shell-navigation-collapsed", async ({page}) => {
      await page.getByRole("button", {name: "Collapse navigation"}).click()
      await expect(page.getByRole("button", {name: "Expand navigation"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-shell-navigation-collapsed")
    })

    test("loc-auth-token-optional", async ({page}) => {
      await prepareVisualPage(page, {app: "localnet", theme: "dark"})
      await page.reload()
      await page.getByRole("button", {name: "Set localnet API token"}).click()
      await expect(page.getByRole("dialog", {name: "Localnet API token"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-auth-token-optional")
    })

    test("loc-modal-advance-time-dark", async ({page}) => {
      await page.route(
        url => url.pathname === "/acton_nodeInfo",
        async route =>
          route.fulfill({
            json: {
              current_unix_time: 1_784_192_438,
              time_offset_seconds: 0,
              next_block_timestamp: null,
              uptime_seconds: 42,
              last_block_seqno: 2,
              state_source: "snapshot",
              fork_network: null,
              fork_block_number: null,
              network_conditions: {response_delay_ms: 0},
            },
          }),
      )
      await prepareVisualPage(page, {app: "localnet", theme: "dark"})
      await page.goto("/")
      await page.getByRole("button", {name: "Advance node time"}).click()
      await expect(page.getByRole("dialog", {name: "Advance time"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-modal-advance-time-dark")
    })

    test("loc-modal-asset-dark", async ({page}) => {
      await prepareVisualPage(page, {app: "localnet", theme: "dark"})
      await page.goto("/faucet")
      await page.getByRole("button", {name: "Choose faucet asset"}).click()
      await expect(page.getByRole("dialog", {name: "Asset"})).toBeVisible()
      await expect(page.getByText("Loading local jettons...")).toHaveCount(0)
      await expectVisualSnapshot(page, "loc-modal-asset-dark")
    })

    test("loc-shell-explorer-dark", async ({page}) => {
      await prepareVisualPage(page, {app: "localnet", theme: "dark"})
      await page.reload()
      await expectVisualSnapshot(page, "loc-shell-explorer-dark")
    })

    test("loc-shell-explorer-mobile", async ({page}) => {
      await page.setViewportSize({width: 390, height: 844})
      await expect(page.getByRole("button", {name: "Open navigation menu"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-shell-explorer-mobile")
    })
  })
})
