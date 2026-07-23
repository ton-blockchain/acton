import {expect, test} from "@playwright/test"

import {prepareVisualPage} from "../support/visual"

const ADDRESS = `0:${"11".repeat(32)}`
const MAINNET_ADDRESS = "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACgQ"
const DEVICE_UID = "12345678-1234-1234-1234-123456789abc"
const ADDRESS_HISTORY_KEY = "actonscanFaucetAddressHistory"
const REQUEST_HISTORY_KEY = "actonscanFaucetRequestHistory"
const SESSION_KEY = "actonscanFaucetSession"
const AUTH_STATUS_URL = "https://faucet.acton.monster/auth/status"

test.describe("Testnet faucet", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {
      app: "explorer",
      storage: {actonscanFaucetDeviceUid: DEVICE_UID},
    })
    await page.route(AUTH_STATUS_URL, async route => {
      const origin = route.request().headers().origin ?? "*"
      if (route.request().method() === "OPTIONS") {
        await route.fulfill({
          status: 204,
          headers: faucetCorsHeaders(origin),
        })
        return
      }
      await route.fulfill({
        contentType: "application/json",
        headers: {"access-control-allow-origin": origin},
        body: JSON.stringify(authStatus(false)),
      })
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
    const walletFundingGuide = page.getByRole("link", {
      name: "documentation",
    })
    await expect(cliGuide).toBeVisible()
    await expect(cliCommand).toBeHidden()
    await cliGuide.click()
    await expect(cliCommand).toHaveText("acton wallet airdrop <WALLET_NAME>")
    await expect(cliCommand).toBeVisible()
    await expect(walletFundingGuide).toHaveAttribute(
      "href",
      "https://ton-blockchain.github.io/acton/docs/wallets#fund-a-wallet-on-testnet",
    )
    await expect(walletFundingGuide).toHaveAttribute("target", "_blank")
    await expect(page.getByText("Wallet airdrop", {exact: true})).toHaveCount(0)
  })

  test("makes the Testnet scope explicit when Mainnet is selected", async ({page}) => {
    await page.goto("/faucet")

    await expect(page.getByText("Mainnet selected")).toBeVisible()
    await expect(page.getByText("Faucet payouts are always sent on Testnet")).toBeVisible()
    await expect(page.getByRole("button", {name: "Get testnet GRAM"})).toBeDisabled()
    await page.getByRole("button", {name: "Switch to Testnet"}).click()

    await expect(page).toHaveURL(/\/faucet\?network=testnet$/)
    await expect(page.getByText("Mainnet selected")).toHaveCount(0)
    await expect(page.getByRole("button", {name: "Testnet", exact: true})).toBeVisible()
    await expect(page.getByRole("button", {name: "Get testnet GRAM"})).toBeEnabled()
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

  test("keeps request history until the server window is known", async ({page}) => {
    const requestAt = Date.now() - 2 * 60 * 60 * 1000
    await page.addInitScript(
      ({storageKey, timestamp}) => {
        localStorage.setItem(storageKey, JSON.stringify([timestamp]))
      },
      {storageKey: REQUEST_HISTORY_KEY, timestamp: requestAt},
    )
    await page.unroute(AUTH_STATUS_URL)
    await page.route(AUTH_STATUS_URL, async route => {
      const origin = route.request().headers().origin ?? "*"
      if (route.request().method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      await route.fulfill({
        contentType: "application/json",
        headers: {"access-control-allow-origin": origin},
        body: JSON.stringify({...authStatus(false), windowSeconds: 24 * 60 * 60}),
      })
    })

    await page.goto("/faucet?network=testnet")

    await expect(page.getByText(/1 of 2 requests used · last request at/)).toBeVisible()
    expect(await page.evaluate(key => localStorage.getItem(key), REQUEST_HISTORY_KEY)).toBe(
      JSON.stringify([requestAt]),
    )
  })

  test("keeps the GitHub session token when auth status is temporarily unavailable", async ({
    page,
  }) => {
    const token = "opaque-session-token-with-enough-entropy"
    await page.addInitScript(({storageKey, value}) => sessionStorage.setItem(storageKey, value), {
      storageKey: SESSION_KEY,
      value: token,
    })
    await page.unroute(AUTH_STATUS_URL)
    await page.route(AUTH_STATUS_URL, async route => {
      const origin = route.request().headers().origin ?? "*"
      if (route.request().method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      await route.fulfill({
        status: 503,
        contentType: "application/json",
        headers: {"access-control-allow-origin": origin},
        body: JSON.stringify({error: "Temporarily unavailable"}),
      })
    })

    const statusResponse = page.waitForResponse(
      response =>
        response.url() === AUTH_STATUS_URL &&
        response.request().method() === "GET" &&
        response.status() === 503,
    )
    await page.goto("/faucet?network=testnet")
    await statusResponse

    await expect
      .poll(() => page.evaluate(key => sessionStorage.getItem(key), SESSION_KEY))
      .toBe(token)
  })

  test("keeps an unused GitHub grant when auth status is temporarily unavailable", async ({
    page,
  }) => {
    await page.unroute(AUTH_STATUS_URL)
    await page.route(AUTH_STATUS_URL, async route => {
      const origin = route.request().headers().origin ?? "*"
      if (route.request().method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      await route.fulfill({
        status: 503,
        contentType: "application/json",
        headers: {"access-control-allow-origin": origin},
        body: JSON.stringify({error: "Temporarily unavailable"}),
      })
    })

    const statusResponse = page.waitForResponse(
      response =>
        response.url() === AUTH_STATUS_URL &&
        response.request().method() === "GET" &&
        response.status() === 503,
    )
    await page.goto("/faucet?network=testnet#github_grant=retryable-grant")
    await statusResponse

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("GitHub connection failed")
    await expect.poll(() => new URL(page.url()).hash).toBe("#github_grant=retryable-grant")
  })

  test("keeps the GitHub session token when session refresh temporarily fails", async ({page}) => {
    const token = "opaque-session-token-with-enough-entropy"
    await page.addInitScript(({storageKey, value}) => sessionStorage.setItem(storageKey, value), {
      storageKey: SESSION_KEY,
      value: token,
    })
    await page.unroute(AUTH_STATUS_URL)
    await page.route("https://faucet.acton.monster/**", async route => {
      const request = route.request()
      const origin = request.headers().origin ?? "*"
      if (request.method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      if (request.url().endsWith("/auth/status")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(authStatus(true)),
        })
        return
      }
      if (request.url().endsWith("/auth/session")) {
        await route.fulfill({
          status: 503,
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify({error: "Temporarily unavailable"}),
        })
        return
      }
      await route.abort()
    })

    const sessionResponse = page.waitForResponse(
      response =>
        response.url().endsWith("/auth/session") &&
        response.request().method() === "GET" &&
        response.status() === 503,
    )
    await page.goto("/faucet?network=testnet")
    await sessionResponse

    await expect
      .poll(() => page.evaluate(key => sessionStorage.getItem(key), SESSION_KEY))
      .toBe(token)
    await expect(page.getByRole("heading", {name: "Higher limits"})).toBeVisible()
    await expect(page.getByRole("button", {name: "Connect GitHub"})).toBeVisible()
  })

  test("reports validation errors through notifications", async ({page}) => {
    await page.goto("/faucet?network=testnet")
    await page.getByLabel("TON address").fill("not-an-address")
    await page.getByRole("button", {name: "Get testnet GRAM"}).click()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Invalid address")
    await expect(notifications).toContainText("Enter a valid TON address")
  })

  test("exchanges a GitHub grant and restores the higher tier session", async ({page}) => {
    await page.unroute(AUTH_STATUS_URL)
    await page.route("https://faucet.acton.monster/**", async route => {
      const request = route.request()
      const origin = request.headers().origin ?? "*"
      if (request.method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      if (request.url().endsWith("/auth/status")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(authStatus(true)),
        })
        return
      }
      if (request.url().endsWith("/auth/exchange")) {
        expect(request.headers()["x-device-uid"]).toBe(DEVICE_UID)
        expect(request.postDataJSON()).toEqual({grant: "one-time-grant"})
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(githubSessionResponse("opaque-session-token-with-enough-entropy")),
        })
        return
      }
      if (request.url().endsWith("/auth/session")) {
        expect(request.headers().authorization).toBe(
          "Bearer opaque-session-token-with-enough-entropy",
        )
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(githubSessionResponse()),
        })
        return
      }
      await route.abort()
    })

    await page.goto("/faucet?network=testnet#github_grant=one-time-grant")

    await expect(page.getByText("Connected as @acton-dev")).toBeVisible()
    await expect(page.getByText("Verified tier · 4 requests per hour")).toBeVisible()
    await expect(page.getByText("Maximum of 4 requests per hour")).toBeVisible()
    await expect(page).toHaveURL(/\/faucet\?network=testnet$/)
    expect(await page.evaluate(key => sessionStorage.getItem(key), SESSION_KEY)).toBe(
      "opaque-session-token-with-enough-entropy",
    )

    await page.reload()
    await expect(page.getByText("Connected as @acton-dev")).toBeVisible()
    await expect(page.getByRole("button", {name: "Disconnect"})).toBeVisible()
  })

  test("does not retain a token from a malformed GitHub grant response", async ({page}) => {
    const token = "opaque-session-token-with-enough-entropy"
    await page.unroute(AUTH_STATUS_URL)
    await page.route("https://faucet.acton.monster/**", async route => {
      const request = route.request()
      const origin = request.headers().origin ?? "*"
      if (request.method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      if (request.url().endsWith("/auth/status")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(authStatus(true)),
        })
        return
      }
      if (request.url().endsWith("/auth/exchange")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify({...githubSessionResponse(token), login: 42}),
        })
        return
      }
      await route.abort()
    })

    await page.goto("/faucet?network=testnet#github_grant=one-time-grant")

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("GitHub connection failed")
    expect(await page.evaluate(key => sessionStorage.getItem(key), SESSION_KEY)).toBeNull()
    await expect.poll(() => new URL(page.url()).hash).toBe("")
  })

  test("restores the address and network after the GitHub redirect", async ({page}) => {
    await page.unroute(AUTH_STATUS_URL)
    await page.route(AUTH_STATUS_URL, async route => {
      const origin = route.request().headers().origin ?? "*"
      if (route.request().method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      await route.fulfill({
        contentType: "application/json",
        headers: {"access-control-allow-origin": origin},
        body: JSON.stringify(authStatus(true)),
      })
    })
    await page.route("https://faucet.acton.monster/auth/github/start?**", async route => {
      const callbackUrl = new URL("/faucet#github_error=access_denied", page.url()).toString()
      await route.fulfill({
        status: 302,
        headers: {location: callbackUrl},
      })
    })

    await page.goto("/faucet?network=testnet")
    await page.getByLabel("TON address").fill(ADDRESS)
    await page.getByRole("button", {name: "Connect GitHub"}).click()

    await expect(page.getByLabel("TON address")).toHaveValue(ADDRESS)
    await expect(page.getByRole("button", {name: "Testnet", exact: true})).toBeVisible()
    await expect
      .poll(() => {
        const url = new URL(page.url())
        return {
          pathname: url.pathname,
          network: url.searchParams.get("network"),
          address: url.searchParams.get("address"),
          hash: url.hash,
        }
      })
      .toEqual({
        pathname: "/faucet",
        network: "testnet",
        address: ADDRESS,
        hash: "",
      })
  })

  test("keeps the connected UI and token when disconnect temporarily fails", async ({page}) => {
    const token = "opaque-session-token-with-enough-entropy"
    let notifyDeleteRequested: () => void
    const deleteRequested = new Promise<void>(resolve => {
      notifyDeleteRequested = resolve
    })
    let releaseDelete: () => void
    const deleteRelease = new Promise<void>(resolve => {
      releaseDelete = resolve
    })
    await page.addInitScript(({storageKey, value}) => sessionStorage.setItem(storageKey, value), {
      storageKey: SESSION_KEY,
      value: token,
    })
    await page.unroute(AUTH_STATUS_URL)
    await page.route("https://faucet.acton.monster/**", async route => {
      const request = route.request()
      const origin = request.headers().origin ?? "*"
      if (request.method() === "OPTIONS") {
        await route.fulfill({status: 204, headers: faucetCorsHeaders(origin)})
        return
      }
      if (request.url().endsWith("/auth/status")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(authStatus(true)),
        })
        return
      }
      if (request.url().endsWith("/auth/session") && request.method() === "GET") {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify(githubSessionResponse()),
        })
        return
      }
      if (request.url().endsWith("/auth/session") && request.method() === "DELETE") {
        notifyDeleteRequested()
        await deleteRelease
        await route.fulfill({
          status: 503,
          contentType: "application/json",
          headers: {"access-control-allow-origin": origin},
          body: JSON.stringify({error: "Temporarily unavailable"}),
        })
        return
      }
      await route.abort()
    })

    await page.goto("/faucet?network=testnet")
    await expect(page.getByText("Connected as @acton-dev")).toBeVisible()
    await page.getByRole("button", {name: "Disconnect"}).click()
    await deleteRequested
    await expect(page.getByRole("button", {name: "Get testnet GRAM"})).toBeDisabled()
    releaseDelete()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Could not disconnect GitHub")
    await expect(page.getByText("Connected as @acton-dev")).toBeVisible()
    expect(await page.evaluate(key => sessionStorage.getItem(key), SESSION_KEY)).toBe(token)
  })

  test("rejects mainnet-friendly addresses before requesting a challenge", async ({page}) => {
    let faucetRequests = 0
    await page.route("https://faucet.acton.monster/**", async route => {
      const path = new URL(route.request().url()).pathname
      if (path === "/challenge" || path === "/claim") {
        faucetRequests += 1
        await route.abort()
        return
      }
      await route.fallback()
    })

    await page.goto("/faucet?network=testnet")
    await page.getByLabel("TON address").fill(MAINNET_ADDRESS)
    await page.getByRole("button", {name: "Get testnet GRAM"}).click()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Mainnet address")
    await expect(notifications).toContainText("Enter a Testnet address (kQ… or 0Q…)")
    expect(faucetRequests).toBe(0)
  })

  test("solves the browser challenge and submits a claim", async ({page}) => {
    let balanceRequests = 0
    let challengeAddress: string | undefined
    const priorRequestAt = Date.now() - 2 * 60 * 60 * 1000
    await page.addInitScript(
      ({sessionKey, requestHistoryKey, token, requestAt}) => {
        sessionStorage.setItem(sessionKey, token)
        localStorage.setItem(requestHistoryKey, JSON.stringify([requestAt]))
      },
      {
        sessionKey: SESSION_KEY,
        requestHistoryKey: REQUEST_HISTORY_KEY,
        token: "opaque-session-token-with-enough-entropy",
        requestAt: priorRequestAt,
      },
    )

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
          headers: faucetCorsHeaders(requestOrigin),
        })
        return
      }
      if (request.url().endsWith("/auth/status")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": requestOrigin},
          body: JSON.stringify({...authStatus(true), windowSeconds: 24 * 60 * 60}),
        })
        return
      }
      if (request.url().endsWith("/auth/session")) {
        await route.fulfill({
          contentType: "application/json",
          headers: {"access-control-allow-origin": requestOrigin},
          body: JSON.stringify(githubSessionResponse()),
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
    const disconnectGitHub = page.getByRole("button", {name: "Disconnect"})
    await expect(disconnectGitHub).toBeEnabled()
    await page.getByLabel("TON address").fill(ADDRESS)
    await page.getByRole("button", {name: "Get testnet GRAM"}).click()

    const notifications = page.getByRole("region", {name: "Notifications"})
    await expect(notifications).toContainText("Requesting a challenge")
    await expect(disconnectGitHub).toBeDisabled()
    await expect(notifications).toContainText("Testnet GRAM received", {timeout: 10_000})
    await expect(disconnectGitHub).toBeEnabled()
    await expect(notifications).toContainText("Balance increased on TON Testnet")
    const viewOnTestnetLink = notifications.getByRole("link", {name: "View on Testnet"})
    await expect(viewOnTestnetLink).toHaveAttribute("href", /network=testnet/)
    await expect(page.getByText(/2 of 4 requests used · last request at/)).toBeVisible()
    const requestHistory = await page.evaluate(
      storageKey => JSON.parse(localStorage.getItem(storageKey) ?? "[]") as unknown[],
      REQUEST_HISTORY_KEY,
    )
    expect(requestHistory).toHaveLength(2)
    expect(requestHistory[0]).toBe(priorRequestAt)
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

function faucetCorsHeaders(origin: string): Record<string, string> {
  return {
    "access-control-allow-origin": origin,
    "access-control-allow-methods": "GET,POST,DELETE",
    "access-control-allow-headers": "content-type,authorization,x-acton-client,x-device-uid",
  }
}

function authStatus(enabled: boolean): Record<string, unknown> {
  return {
    enabled,
    guestMaxRequests: 2,
    verifiedMaxRequests: 4,
    establishedMaxRequests: 8,
    windowSeconds: 3600,
  }
}

function githubSessionResponse(token?: string): Record<string, unknown> {
  return {
    authenticated: true,
    githubUserId: 42,
    login: "acton-dev",
    tier: "verified",
    maxRequests: 4,
    accountAgeDays: 800,
    publicRepos: 12,
    followers: 7,
    ...(token ? {token} : {}),
  }
}
