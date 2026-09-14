import {expect, test} from "@playwright/test"

const ADDRESS = `0:${"11".repeat(32)}`
const DEVICE_UID = "12345678-1234-1234-1234-123456789abc"
const SESSION_TOKEN = "opaque-session-token-with-enough-entropy"
const AUTH_STATUS = {
  enabled: true,
  guestMaxRequests: 2,
  verifiedMaxRequests: 4,
  establishedMaxRequests: 8,
  windowSeconds: 3600,
}
const SESSION = {
  authenticated: true,
  githubUserId: 42,
  login: "faucet-user",
  tier: "verified",
  maxRequests: 4,
  windowSeconds: 3600,
  publicRepos: 10,
  followers: 2,
  accountAgeDays: 2000,
}

test.beforeEach(async ({page}) => {
  await page.addInitScript(deviceUid => {
    localStorage.setItem("actonscanFaucetDeviceUid", deviceUid)
  }, DEVICE_UID)
  await page.route("**/auth/status", async route => {
    expect(new URL(route.request().url()).origin).toBe(new URL(page.url()).origin)
    await route.fulfill({json: AUTH_STATUS})
  })
})

test("requests GRAM using the same-origin API and the production WASM worker", async ({page}) => {
  let balanceRequests = 0
  let challengeAddress: string | undefined
  await page.route("https://testnet.toncenter.com/api/v2/getAddressInformation*", async route => {
    balanceRequests += 1
    await route.fulfill({
      json: {ok: true, result: {balance: balanceRequests === 1 ? "0" : "1000000000"}},
    })
  })
  await page.route("**/challenge", async route => {
    expect(new URL(route.request().url()).origin).toBe(new URL(page.url()).origin)
    expect(route.request().headers()["x-device-uid"]).toBe(DEVICE_UID)
    challengeAddress = (route.request().postDataJSON() as {address: string}).address
    await route.fulfill({
      json: {
        version: 1,
        challenge: "standalone-faucet-e2e",
        difficulty: 0,
        max_solve_ttl_seconds: 30,
        max_nonce_attempts: 100,
      },
    })
  })
  await page.route("**/claim", async route => {
    expect(new URL(route.request().url()).origin).toBe(new URL(page.url()).origin)
    expect(route.request().postDataJSON()).toMatchObject({
      address: challengeAddress,
      version: 1,
      challenge: "standalone-faucet-e2e",
      nonce: 0,
      type: 1,
    })
    await route.fulfill({json: {message: "Your airdrop is in progress"}})
  })

  await page.goto(`/?address=${encodeURIComponent(ADDRESS)}`)
  await expect(page.getByRole("heading", {name: "Request testnet GRAM"})).toBeVisible()
  await expect(page.getByLabel("TON address")).toHaveValue(ADDRESS)
  await expect(page.getByRole("region", {name: "GitHub faucet limits"})).toContainText(
    "Connect GitHub to unlock up to 8 requests per hour",
  )
  await expect(page.getByRole("button", {name: "Connect GitHub"})).toBeEnabled()
  await expect(page.getByRole("button", {name: "Switch to Testnet"})).toHaveCount(0)
  await page.getByRole("button", {name: "Get testnet GRAM"}).click()

  const notifications = page.getByRole("region", {name: "Notifications"})
  await expect(notifications).toContainText("Testnet GRAM received", {timeout: 10_000})
  await expect(notifications.getByRole("link", {name: "View on Testnet"})).toHaveAttribute(
    "href",
    `https://actonscan.com/address/${challengeAddress}?network=testnet`,
  )
  expect(balanceRequests).toBe(2)
})

test("exchanges and disconnects GitHub sessions on the same origin", async ({page}) => {
  let exchanged = false
  let disconnected = false
  await page.route("**/auth/exchange", async route => {
    expect(new URL(route.request().url()).origin).toBe(new URL(page.url()).origin)
    expect(route.request().postDataJSON()).toEqual({grant: "standalone-grant"})
    exchanged = true
    await route.fulfill({json: {...SESSION, token: SESSION_TOKEN}})
  })
  await page.route("**/auth/session", async route => {
    expect(new URL(route.request().url()).origin).toBe(new URL(page.url()).origin)
    expect(route.request().headers().authorization).toBe(`Bearer ${SESSION_TOKEN}`)
    if (route.request().method() === "DELETE") {
      disconnected = true
      await route.fulfill({status: 204})
    } else {
      await route.fulfill({json: SESSION})
    }
  })

  await page.goto("/#github_grant=standalone-grant")
  await expect(page.getByRole("button", {name: "Disconnect"})).toBeEnabled()
  expect(exchanged).toBe(true)
  await expect.poll(() => new URL(page.url()).hash).toBe("")
  await page.reload()
  await page.getByRole("button", {name: "Disconnect"}).click()
  await expect(page.getByRole("region", {name: "Notifications"})).toContainText(
    "GitHub disconnected",
  )
  expect(disconnected).toBe(true)

  await page.route("**/auth/github/start?*", async route => {
    await route.fulfill({contentType: "text/html", body: "GitHub authorization"})
  })
  const {origin} = new URL(page.url())
  await page.getByRole("button", {name: "Connect GitHub"}).click()
  await expect(page).toHaveURL(`${origin}/auth/github/start?device_uid=${DEVICE_UID}`)
})
