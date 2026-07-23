import {expect, test} from "@playwright/test"

import {prepareVisualPage} from "../support/visual"

const ADDRESS = `0:${"11".repeat(32)}`
const DEVICE_UID = "12345678-1234-1234-1234-123456789abc"

test.describe("Standalone Testnet faucet", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {
      app: "faucet",
      storage: {actonscanFaucetDeviceUid: DEVICE_UID},
    })
    await page.route("**/auth/status", route =>
      route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          enabled: true,
          guestMaxRequests: 2,
          verifiedMaxRequests: 4,
          establishedMaxRequests: 8,
          windowSeconds: 3600,
        }),
      }),
    )
  })

  test("renders the shared faucet flow without explorer network chrome", async ({page}) => {
    await page.goto("/")

    await expect(page.getByRole("link", {name: "Acton Testnet Faucet home"})).toBeVisible()
    await expect(page.getByRole("heading", {name: "Request testnet GRAM"})).toBeVisible()
    await expect(page.getByText("Mainnet selected")).toHaveCount(0)
    await expect(page.getByLabel("TON address")).toBeFocused()
    await expect(page.getByRole("button", {name: "Get testnet GRAM"})).toBeEnabled()
  })

  test("uses same-origin APIs and confirms the Testnet balance", async ({page}) => {
    let balanceRequests = 0
    await page.route("**/address/*/balance", async route => {
      balanceRequests += 1
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({balance: balanceRequests === 1 ? "0" : "1000000000"}),
      })
    })
    await page.route("**/challenge", async route => {
      expect(route.request().headers()["x-acton-client"]).toBe("acton-faucet-ui/1.0.0")
      expect(route.request().headers()["x-device-uid"]).toBe(DEVICE_UID)
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          version: 1,
          challenge: "standalone-faucet-e2e",
          difficulty: 0,
          max_solve_ttl_seconds: 30,
          max_nonce_attempts: 100,
        }),
      })
    })
    await page.route("**/claim", async route => {
      expect(route.request().postDataJSON()).toMatchObject({
        challenge: "standalone-faucet-e2e",
        nonce: 0,
        type: 1,
        version: 1,
      })
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({message: "Your claim has been queued"}),
      })
    })

    await page.goto("/")
    await page.getByLabel("TON address").fill(ADDRESS)
    await page.getByRole("button", {name: "Get testnet GRAM"}).click()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Testnet GRAM received", {timeout: 10_000})
    await expect(notifications.getByRole("link", {name: "View on Testnet"})).toHaveAttribute(
      "href",
      /^https:\/\/actonscan\.com\/address\/.+network=testnet$/,
    )
    expect(balanceRequests).toBe(2)
  })
})
