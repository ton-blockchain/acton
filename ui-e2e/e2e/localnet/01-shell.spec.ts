import {expect, test} from "@playwright/test"

import {expectVisualSnapshot, prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

test.describe("Localnet shell", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "localnet"})
    await page.goto("/explorer")
    await expect(page.getByRole("complementary", {name: "Main navigation"})).toBeVisible()
    await expect(page.getByPlaceholder("Search by address or hash")).toBeVisible()
    await expect(page.getByText("Failed to load wallets")).toHaveCount(0)
  })

  test("renders explorer inside dashboard navigation", async ({page}) => {
    await expect(page.getByRole("button", {name: "Collapse navigation"})).toBeVisible()
    await expect(page.getByRole("button", {name: "Set localnet API token"})).toBeVisible()
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
      await page.getByRole("button", {name: "Set localnet API token"}).click()
      await expect(page.getByRole("dialog", {name: "Localnet API token"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-auth-token-optional")
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
