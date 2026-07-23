import {expect, test} from "@playwright/test"

import {prepareVisualPage} from "../support/visual"

const ADDRESS = `0:${"11".repeat(32)}`
const DEVICE_UID = "12345678-1234-1234-1234-123456789abc"
const ADDRESS_HISTORY_KEY = "actonscanFaucetAddressHistory"
const REQUEST_HISTORY_KEY = "actonscanFaucetRequestHistory"

test.describe("Testnet faucet", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {
      app: "explorer",
      storage: {actonscanFaucetDeviceUid: DEVICE_UID},
    })
  })

  test("keeps Faucet in the desktop more menu", async ({page}) => {
    await page.goto("/")

    const primaryNavigation = page.getByRole("navigation", {name: "Explorer navigation"})
    await expect(primaryNavigation.getByRole("link", {name: "Faucet"})).toHaveCount(0)

    await page.getByRole("button", {name: "Open more navigation"}).click()
    const moreNavigation = page.getByRole("navigation", {name: "More explorer navigation"})
    await expect(moreNavigation.getByRole("link", {name: /Faucet/})).toHaveAttribute(
      "href",
      "/faucet",
    )

    await moreNavigation.getByRole("link", {name: /Faucet/}).click()
    await expect(page).toHaveURL(/\/faucet$/)
    await expect(page.getByRole("heading", {name: "Request testnet GRAM"})).toBeVisible()
    await expect(page.getByLabel("TON address")).toHaveAttribute(
      "placeholder",
      "Enter a friendly (kQ…) or raw (0:…) address",
    )
    await expect(page.getByLabel("TON address")).toBeFocused()
    await expect(page.getByText("No wallet connection or private key is required")).toBeVisible()
    const cliGuide = page.getByText("Use Acton CLI")
    const cliCommand = page.locator("code")
    await expect(cliGuide).toBeVisible()
    await expect(cliCommand).toBeHidden()
    await cliGuide.click()
    await expect(cliCommand).toHaveText("acton wallet airdrop <WALLET_NAME>")
    await expect(cliCommand).toBeVisible()
    await expect(page.getByText("Wallet airdrop", {exact: true})).toHaveCount(0)
  })

  test("makes the Testnet scope explicit when Mainnet is selected", async ({page}) => {
    await page.goto("/faucet")

    await expect(page.getByText("Mainnet selected")).toBeVisible()
    await expect(page.getByText("Faucet payouts are always sent on Testnet")).toBeVisible()
    await page.getByRole("button", {name: "Switch to Testnet"}).click()

    await expect(page).toHaveURL(/\/faucet\?network=testnet$/)
    await expect(page.getByText("Mainnet selected")).toHaveCount(0)
    await expect(page.getByRole("button", {name: "Testnet", exact: true})).toBeVisible()
  })

  test("disables requests when the local hourly history reaches the limit", async ({page}) => {
    const now = Date.now()
    await page.addInitScript(
      ({storageKey, timestamps}) => {
        localStorage.setItem(storageKey, JSON.stringify(timestamps))
      },
      {
        storageKey: REQUEST_HISTORY_KEY,
        timestamps: [now - 20_000, now - 10_000],
      },
    )

    await page.goto("/faucet?network=testnet")

    await expect(page.getByText(/2 of 2 requests used · available again at/)).toBeVisible()
    await expect(page.getByRole("button", {name: /Available again at/})).toBeDisabled()
  })

  test("reports validation errors through notifications", async ({page}) => {
    await page.goto("/faucet?network=testnet")
    await page.getByLabel("TON address").fill("not-an-address")
    await page.getByRole("button", {name: "Get testnet GRAM"}).click()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Invalid address")
    await expect(notifications).toContainText("Enter a valid TON address")
  })

  test("solves the browser challenge and submits a claim", async ({page}) => {
    let balanceRequests = 0
    let challengeAddress: string | undefined

    await page.route("https://testnet.toncenter.com/api/v3/addressInformation*", async route => {
      balanceRequests += 1
      await route.fulfill({
        contentType: "application/json",
        headers: {"access-control-allow-origin": "*"},
        body: JSON.stringify({balance: balanceRequests === 1 ? "0" : "1000000000"}),
      })
    })
    await page.route("https://faucet.acton.monster/**", async route => {
      const request = route.request()
      const requestOrigin = request.headers().origin ?? "*"
      if (request.method() === "OPTIONS") {
        await route.fulfill({
          status: 204,
          headers: {
            "access-control-allow-origin": requestOrigin,
            "access-control-allow-methods": "POST",
            "access-control-allow-headers": "content-type,x-acton-client,x-device-uid",
          },
        })
        return
      }

      expect(request.headers()["x-acton-client"]).toBe("actonscan/1.0.0")
      expect(request.headers()["x-device-uid"]).toBe(DEVICE_UID)
      const payload = request.postDataJSON() as Record<string, unknown>
      const corsHeaders = {"access-control-allow-origin": requestOrigin}

      if (request.url().endsWith("/challenge")) {
        challengeAddress = String(payload.address)
        expect(payload.type).toBe(1)
        await new Promise(resolve => setTimeout(resolve, 300))
        await route.fulfill({
          contentType: "application/json",
          headers: corsHeaders,
          body: JSON.stringify({
            version: 1,
            challenge: "actonscan-e2e",
            difficulty: 0,
            max_solve_ttl_seconds: 30,
            max_nonce_attempts: 100,
          }),
        })
        return
      }

      expect(request.url()).toMatch(/\/claim$/)
      expect(payload).toMatchObject({
        address: challengeAddress,
        version: 1,
        challenge: "actonscan-e2e",
        nonce: 0,
        type: 1,
      })
      await route.fulfill({
        contentType: "application/json",
        headers: corsHeaders,
        body: JSON.stringify({message: "Your airdrop is in progress"}),
      })
    })

    await page.goto("/faucet?network=testnet")
    await page.getByLabel("TON address").fill(ADDRESS)
    await page.getByRole("button", {name: "Get testnet GRAM"}).click()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Requesting a challenge")
    await expect(notifications).toContainText("Testnet GRAM received", {timeout: 10_000})
    await expect(notifications).toContainText("Balance increased on TON Testnet")
    const viewOnTestnetLink = notifications.getByRole("link", {name: "View on Testnet"})
    await expect(viewOnTestnetLink).toHaveAttribute("href", /network=testnet/)
    await expect(page.getByText(/1 of 2 requests used · last request at/)).toBeVisible()
    const requestHistory = await page.evaluate(
      storageKey => JSON.parse(localStorage.getItem(storageKey) ?? "[]") as unknown[],
      REQUEST_HISTORY_KEY,
    )
    expect(requestHistory).toHaveLength(1)
    expect(challengeAddress).toBeTruthy()
    expect(balanceRequests).toBe(2)
    if (!challengeAddress) {
      throw new Error("The faucet challenge address was not captured")
    }

    const addressHistory = await page.evaluate(
      storageKey => JSON.parse(localStorage.getItem(storageKey) ?? "[]") as unknown[],
      ADDRESS_HISTORY_KEY,
    )
    expect(addressHistory).toEqual([challengeAddress])

    const addressInput = page.getByLabel("TON address")
    await addressInput.fill("")
    await addressInput.click()
    const historyOption = page.getByRole("option").filter({hasText: challengeAddress})
    await expect(historyOption).toBeVisible()
    await historyOption.click()
    await expect(addressInput).toHaveValue(challengeAddress)

    await addressInput.fill("")
    await addressInput.click()
    await page.getByRole("button", {name: `Remove ${challengeAddress} from history`}).click()
    await expect(historyOption).toHaveCount(0)
    const addressHistoryAfterRemoval = await page.evaluate(
      storageKey => JSON.parse(localStorage.getItem(storageKey) ?? "[]") as unknown[],
      ADDRESS_HISTORY_KEY,
    )
    expect(addressHistoryAfterRemoval).toEqual([])

    await page.evaluate(() => {
      document.documentElement.dataset.faucetSpaNavigation = "preserved"
    })
    await viewOnTestnetLink.click()
    await expect(page).toHaveURL(/\/address\/.+\?network=testnet$/)
    await expect
      .poll(() => page.evaluate(() => document.documentElement.dataset.faucetSpaNavigation))
      .toBe("preserved")
  })
})
