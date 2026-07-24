import {expect, test, type Page} from "@playwright/test"

import {mockTonConnectStartupWallet} from "../support/tonConnect"
import {expectVisualSnapshot, prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

const demoTonConnectUrl =
  "tc://?v=2&id=3b1cba70841d3695092dc3792246ffb6cb76398be67a5684b5c570dc85c4e172&trace_id=019f7f09-9036-7416-8c01-0c47c208000e&r=%7B%22manifestUrl%22%3A%22https%3A%2F%2Ftonconnect-sdk-demo-dapp.vercel.app%2Ftonconnect-manifest.json%22%2C%22items%22%3A%5B%7B%22name%22%3A%22ton_addr%22%7D%5D%7D&ret=none"
const mockTonConnectRequest = async (page: Page) => {
  await page.route(
    "https://tonconnect-sdk-demo-dapp.vercel.app/tonconnect-manifest.json",
    async route =>
      route.fulfill({
        json: {
          url: "https://tonconnect-sdk-demo-dapp.vercel.app",
          name: "Demo Dapp with React UI",
          iconUrl: "https://tonconnect-sdk-demo-dapp.vercel.app/favicon.ico",
        },
      }),
  )
  await mockTonConnectStartupWallet(
    page,
    "0:3029b3eaeda86a5381d86100f2a8b761c38de45642edb6e4bb1cca2e6dd7ffed",
  )
}

const openTonConnectRequest = async (page: Page) => {
  await mockTonConnectRequest(page)
  await page.goto("/wallets")
  await expect(page.getByLabel("Connect URL")).toBeEnabled()
  await page.getByLabel("Connect URL").fill(demoTonConnectUrl)
  await page.getByRole("button", {name: "Handle request"}).click()
  await expect(page.getByRole("dialog", {name: "Connection Request"})).toBeVisible()
}

test.describe("Localnet shell", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "localnet"})
    await page.goto("/explorer")
    await expect(page.getByRole("complementary", {name: "Main navigation"})).toBeVisible()
    await expect(page.getByRole("combobox", {name: "Explorer search"})).toBeVisible()
    await expect(page.getByText("Failed to load wallets")).toHaveCount(0)
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

  test("opens a TON Connect connection request", async ({page}) => {
    await openTonConnectRequest(page)

    const dialog = page.getByRole("dialog", {name: "Connection Request"})
    await expect(
      dialog.getByText("Demo Dapp with React UI wants to connect", {exact: true}),
    ).toBeVisible()
    await expect(dialog.getByText("Connect with", {exact: true})).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Reject"})).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Connect", exact: true})).toBeVisible()
    await expect(
      page.locator('[data-variant="info"]').filter({hasText: "TON Connect request received"}),
    ).toBeVisible()
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

    test("loc-modal-ton-connect-dark", async ({page}) => {
      await prepareVisualPage(page, {app: "localnet", theme: "dark"})
      await openTonConnectRequest(page)
      await expectVisualSnapshot(page, "loc-modal-ton-connect-dark")
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

test("does not duplicate the global wallet error toast", async ({page}) => {
  await prepareVisualPage(page, {app: "localnet"})
  await page.route(
    url => url.pathname === "/acton_getStartupWallets",
    async route => route.abort("connectionfailed"),
  )

  await page.goto("/explorer")

  const errorToast = page.locator('[data-variant="error"]').filter({
    hasText: "Failed to load wallets",
  })
  await expect(errorToast).toHaveCount(1)
  await page.waitForTimeout(500)
  await expect(errorToast).toHaveCount(1)
})
