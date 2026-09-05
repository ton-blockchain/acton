import {expect, test, type BrowserContext, type Locator, type Page} from "@playwright/test"

import {
  expectVisualElementSnapshot,
  prepareVisualPage,
  visualSnapshotsEnabled,
} from "../support/visual"

const DAPP_URL = "http://127.0.0.1:14308"
const DAPP_DOMAIN = "appkit-minter-chee5uqvk-topteam.vercel.app"
const ENVIRONMENT_NAME = "TON Connect visual checks"

let environmentId = ""

test.describe("Studio TON Connect approvals", () => {
  test.beforeAll(async ({request}) => {
    const existingResponse = await request.get("/api/v1/environments")
    expect(existingResponse.ok()).toBe(true)

    const existing = (await existingResponse.json()) as Array<{
      readonly id: string
      readonly name: string
    }>
    for (const environment of existing.filter(item => item.name === ENVIRONMENT_NAME)) {
      const deleteResponse = await request.delete(`/api/v1/environments/${environment.id}`)
      expect(deleteResponse.ok()).toBe(true)
    }

    const createResponse = await request.post("/api/v1/environments", {
      data: {
        name: ENVIRONMENT_NAME,
        config: {
          kind: "actonSimulatedLocalnet",
          accounts: ["visual-wallet"],
          noMining: true,
          mineEmptyBlocks: false,
        },
      },
    })
    expect(createResponse.ok()).toBe(true)

    const created = (await createResponse.json()) as {readonly id: string}
    environmentId = created.id

    await expect
      .poll(
        async () => {
          const response = await request.get("/api/v1/environments")
          const environments = (await response.json()) as Array<{
            readonly id: string
            readonly status: string
          }>
          return environments.find(environment => environment.id === environmentId)?.status
        },
        {timeout: 60_000},
      )
      .toBe("running")
  })

  test.afterAll(async ({request}) => {
    if (environmentId) {
      await request.delete(`/api/v1/environments/${environmentId}`)
    }
  })

  test.beforeEach(async ({context}) => {
    // Keep the long manifest origin from the design reference while using only local transport
    await context.route(`${DAPP_URL}/tonconnect-manifest.json`, route =>
      route.fulfill({
        json: {
          url: `https://${DAPP_DOMAIN}`,
          name: "Appkit Minter",
          iconUrl: `${DAPP_URL}/icon.svg`,
        },
      }),
    )
  })

  test("shows the dApp, selected wallet, permissions, and safety context", async ({
    context,
    page,
  }) => {
    const dappPage = await openConnectionRequest(page, context)

    const dialog = page.getByRole("dialog", {name: /Connect to/})
    await expect(
      dialog.getByRole("heading", {name: `Connect to ${DAPP_DOMAIN}?`, exact: true}),
    ).toBeVisible()
    await expect(
      dialog.getByText("Appkit Minter is requesting access to your wallet address:"),
    ).toBeVisible()
    await selectVisualWallet(dialog)
    await expect(dialog.getByText("visual-wallet", {exact: true})).toBeVisible()
    await expect(dialog.getByRole("heading", {name: "Permissions"})).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Connect wallet"})).toBeEnabled()
    await expect(dialog.getByRole("button", {name: "Reject"})).toBeEnabled()

    if (visualSnapshotsEnabled) {
      await expectVisualElementSnapshot(page, dialog, "studio-ton-connect-request")
    }

    await dialog.getByRole("button", {name: "Reject"}).click()
    await dappPage.close()
  })

  test("shows every transaction recipient and the total before approval", async ({
    context,
    page,
  }) => {
    const dappPage = await connectDapp(page, context)

    await dappPage.getByRole("button", {name: "Request transaction"}).click()
    const dialog = page.getByRole("dialog", {name: /Confirm transaction for/})
    await expect(
      dialog.getByRole("heading", {name: `Confirm transaction for ${DAPP_DOMAIN}?`, exact: true}),
    ).toBeVisible()
    await expect(
      dialog.getByText("A dApp wants to send a transaction from your wallet:"),
    ).toBeVisible()
    await expect(dialog.getByText("2.75")).toBeVisible()
    await expect(dialog.getByText("#1", {exact: true})).toBeVisible()
    await expect(dialog.getByText("#2", {exact: true})).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Approve and sign"})).toBeEnabled()

    if (visualSnapshotsEnabled) {
      await expectVisualElementSnapshot(page, dialog, "studio-ton-connect-transaction")
    }

    await context.grantPermissions(["clipboard-read", "clipboard-write"])
    await dialog.getByRole("button", {name: "Copy address", exact: true}).first().click()
    await expect(dialog.getByRole("button", {name: "Address copied"})).toBeVisible()
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
      "kQBBJBB3HagsujBqVfqeDUPJ0kXjgTPLWPFFffuNXNiJL_0K",
    )

    await dialog.getByRole("button", {name: "Reject"}).click()
    await dappPage.close()
  })

  test("uses the same approval shell for sign data", async ({context, page}) => {
    const dappPage = await connectDapp(page, context)

    await dappPage.getByRole("button", {name: "Request text signature"}).click()
    const dialog = page.getByRole("dialog", {name: /Sign data for/})
    await expect(dialog.getByRole("heading", {name: /Sign data for/})).toBeVisible()
    await expect(dialog.getByText("Text", {exact: true})).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Sign data"})).toBeEnabled()

    if (visualSnapshotsEnabled) {
      await expectVisualElementSnapshot(page, dialog, "studio-ton-connect-sign-data")
    }

    await dialog.getByRole("button", {name: "Reject"}).click()
    await dappPage.close()
  })

  test("keeps the cell inspector and signing actions reachable below the header", async ({
    context,
    page,
  }) => {
    const dappPage = await connectDapp(page, context)
    await dappPage.getByRole("button", {name: "Request cell signature"}).click()

    const dialog = page.getByRole("dialog", {name: /Sign data for/})
    await expect(dialog.getByText("ActonSignRequest", {exact: true}).first()).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Expand Schema", exact: true})).toBeVisible()
    await expect(dialog.getByRole("button", {name: "Sign data"})).toBeInViewport()
    await dialog.getByRole("heading", {name: /Sign data for/}).scrollIntoViewIfNeeded()

    if (visualSnapshotsEnabled) {
      await expectVisualElementSnapshot(page, dialog, "studio-ton-connect-sign-cell")
    }

    await dialog.getByRole("button", {name: "Reject"}).click()
    await dappPage.close()
  })

  for (const theme of ["light", "dark"] as const) {
    test(`wraps long domains on mobile in the ${theme} theme`, async ({context, page}) => {
      await page.setViewportSize({width: 390, height: 844})
      const dappPage = await openConnectionRequest(page, context, theme)
      const dialog = page.getByRole("dialog", {name: /Connect to/})

      await expect(dialog).toHaveAttribute("data-theme", theme)
      await selectVisualWallet(dialog)
      await dialog.getByRole("heading", {name: /Connect to/}).scrollIntoViewIfNeeded()
      await expect(dialog.getByRole("button", {name: "Connect wallet"})).toBeInViewport()
      await expect(dialog.getByRole("button", {name: "Reject"})).toBeInViewport()
      expect(await dialog.evaluate(element => element.scrollWidth > element.clientWidth)).toBe(
        false,
      )

      if (visualSnapshotsEnabled) {
        await expectVisualElementSnapshot(page, dialog, `studio-ton-connect-mobile-${theme}`)
      }

      // The header and content must scroll together, with both actions pinned even in landscape
      await page.setViewportSize({width: 667, height: 375})
      await dialog.getByRole("heading", {name: "Permissions"}).scrollIntoViewIfNeeded()
      await expect(dialog.getByRole("button", {name: "Connect wallet"})).toBeInViewport()
      await expect(dialog.getByRole("button", {name: "Reject"})).toBeInViewport()
      await dialog.getByRole("button", {name: "Reject"}).click()
      await dappPage.close()
    })
  }
})

async function connectDapp(page: Page, context: BrowserContext): Promise<Page> {
  const dappPage = await openConnectionRequest(page, context)
  const dialog = page.getByRole("dialog", {name: /Connect to/})

  await selectVisualWallet(dialog)
  await dialog.getByRole("button", {name: "Connect wallet"}).click()
  await expect(dialog).toHaveCount(0)
  await expect(dappPage.getByRole("status")).toContainText("Connected:")

  return dappPage
}

async function selectVisualWallet(dialog: Locator): Promise<void> {
  const walletPicker = dialog.getByRole("button", {name: /Change wallet, currently/})
  if ((await walletPicker.count()) === 0) {
    return
  }

  await walletPicker.click()
  await dialog.getByRole("option").filter({hasText: "visual-wallet"}).click()
}

async function openConnectionRequest(
  page: Page,
  context: BrowserContext,
  theme: "light" | "dark" = "light",
): Promise<Page> {
  await prepareVisualPage(page, {app: "studio", theme})
  await page.goto(`/virtual-environments/${environmentId}/wallets`)
  await expect(page.getByText("visual-wallet", {exact: true}).first()).toBeVisible()

  const dappPage = await context.newPage()
  await dappPage.goto(DAPP_URL)
  await dappPage.getByRole("button", {name: "Create connection"}).click()
  const connectionUrl = dappPage.getByLabel("TON Connect URL")
  await expect(connectionUrl).not.toHaveValue("")

  await page.getByLabel("Connect URL").fill(await connectionUrl.inputValue())
  await page.getByRole("button", {name: "Handle request"}).click()
  await expect(page.getByTestId("ton-connect-connect-request")).toBeVisible()

  return dappPage
}
