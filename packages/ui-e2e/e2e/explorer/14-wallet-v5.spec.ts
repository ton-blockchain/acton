import {expect, test, type Page} from "@playwright/test"
import {Address, beginCell, Dictionary} from "@ton/core"

import bundledAbiCatalog from "../../../../crates/acton-abi-catalog/data/data-abis.json" with {
  type: "json",
}
import {prepareVisualPage} from "../support/visual"

const WALLET_ADDRESS = `0:${"17".repeat(32)}`
const PLUGIN_ADDRESSES = [`0:${"39".repeat(32)}`, `0:${"ae".repeat(32)}`]
const WALLET_V5_CODE_HASH = "20834b7b72b112147e1b2fb457b84e74d1a30f04f737d4f62a668e9552d2b72f"
const UNKNOWN_CODE_HASH = "01".repeat(32)
const PLUGIN_CODE_HASH = "04".repeat(32)
const AUTO_ENABLE_HINT = "(auto-enables on signed request)"
const AUTO_ENABLE_EXPLANATION =
  "No plugins are installed, so valid requests signed with the owner's key are still accepted. Processing a signed request automatically enables signature auth."
const WALLET_ABI_RECORD = bundledAbiCatalog.contracts.find(
  contract => contract.id === "wallets.WalletV5r1",
)
if (!WALLET_ABI_RECORD) {
  throw new Error("Wallet V5 ABI fixture is missing from the catalog")
}

function abbreviatedAddress(address: string): string {
  return `${address.slice(0, 6)}…${address.slice(-6)}`
}

function walletData(plugins = PLUGIN_ADDRESSES, signatureAllowed = true): string {
  const extensions = Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.BigInt(1))
  for (const address of plugins) {
    extensions.set(BigInt(`0x${Address.parse(address).hash.toString("hex")}`), -1n)
  }
  return beginCell()
    .storeBit(signatureAllowed)
    .storeUint(12, 32)
    .storeUint(2_147_483_409, 32)
    .storeUint(123, 256)
    .storeDict(extensions)
    .endCell()
    .toBoc()
    .toString("base64")
}

interface WalletFixtureOptions {
  readonly data?: string | null
  readonly codeHash?: string
  readonly interfaces?: readonly string[]
  readonly status?: "active" | "frozen" | "uninitialized"
  readonly pluginTypesFail?: boolean
  readonly pluginTypesReady?: Promise<void>
  readonly missingPluginStates?: readonly string[]
  readonly pluginAbi?: "unavailable" | "missing"
  readonly withWalletAbi?: boolean
}

async function mockWallet(page: Page, options: WalletFixtureOptions = {}) {
  let pluginTypesFail = options.pluginTypesFail ?? false
  let missingPluginStates = options.missingPluginStates ?? []
  let pluginAbi: "unavailable" | "missing" | "available" | undefined = options.pluginAbi
  const pluginTypeRequests: string[][] = []
  const pluginAbiRequests: string[] = []
  const codeHash = options.codeHash ?? UNKNOWN_CODE_HASH
  const interfaces = options.interfaces ?? ["wallet_v5r1"]
  const state = {
    balance: "9876543210",
    code: null,
    data: options.data === undefined ? walletData() : options.data,
    frozen_hash: null,
    last_transaction_hash: "02".repeat(32),
    last_transaction_lt: "100",
    status: options.status ?? "active",
  }
  const emptyResponses: Record<string, unknown> = {
    transactions: {transactions: [], address_book: {}},
    actions: {actions: [], address_book: {}, metadata: {}},
    getConfigParam: {config: {bytes: ""}},
    records: {records: []},
    wallets: {jetton_wallets: [], metadata: {}},
    items: {nft_items: []},
    source: {verified: false, bundle: null},
    abi: {},
  }
  const accountForAddress = (address: string) => {
    const rawAddress = Address.parse(address).toRawString()
    const isWallet = rawAddress === WALLET_ADDRESS
    const pluginInterfaces = rawAddress === PLUGIN_ADDRESSES[0] ? ["multisig_v2"] : []
    const pluginCodeHash =
      options.pluginAbi && rawAddress === PLUGIN_ADDRESSES[1] ? PLUGIN_CODE_HASH : UNKNOWN_CODE_HASH
    return {
      ...state,
      address,
      data: isWallet ? state.data : null,
      account_state_hash: "03".repeat(32),
      code_hash: isWallet ? codeHash : pluginCodeHash,
      interfaces: isWallet ? interfaces : pluginInterfaces,
      contract_methods: [],
      extra_currencies: {},
    }
  }
  await page.route(
    url => url.pathname.startsWith("/api/") || (url.pathname.includes("data-abis") && !url.search),
    async route => {
      const url = new URL(route.request().url())
      const addresses = url.searchParams.getAll("address")
      const primaryAddress = addresses[0] ?? WALLET_ADDRESS
      const accountState = accountForAddress(primaryAddress)
      const isPluginTypeRequest =
        url.pathname.endsWith("/accountStates") &&
        url.searchParams.get("include_boc") === "true" &&
        Address.parse(primaryAddress).toRawString() !== WALLET_ADDRESS
      if (isPluginTypeRequest) {
        pluginTypeRequests.push(addresses.map(address => Address.parse(address).toRawString()))
        await options.pluginTypesReady
        if (pluginTypesFail) {
          await route.fulfill({status: 503, json: {error: "Plugin types unavailable"}})
          return
        }
      }
      const responses: Record<string, unknown> = {
        ...emptyResponses,
        addressInformation: accountState,
        accountStates: {
          accounts: addresses
            .filter(
              address =>
                !(
                  isPluginTypeRequest &&
                  missingPluginStates.includes(Address.parse(address).toRawString())
                ),
            )
            .map(accountForAddress),
          address_book: {},
          metadata: {},
        },
      }
      const response = url.pathname.includes("data-abis")
        ? {schemaVersion: 1, contracts: options.withWalletAbi ? [WALLET_ABI_RECORD] : []}
        : responses[url.pathname.split("/").at(-1) ?? ""]
      if (response === undefined) {
        await route.fulfill({status: 404, json: {error: "No fixture for this endpoint"}})
        return
      }
      await route.fulfill({json: response})
    },
  )
  await page.route(
    url => url.pathname.endsWith("/abi") && url.searchParams.get("code_hash") === PLUGIN_CODE_HASH,
    async route => {
      pluginAbiRequests.push(route.request().url())
      if (pluginAbi === "unavailable") {
        await route.fulfill({status: 503, json: {error: "Verifier unavailable"}})
      } else if (pluginAbi === "missing") {
        await route.fulfill({status: 404, json: {error: "No ABI found"}})
      } else {
        await route.fulfill({
          json: {
            items: [
              {
                code_hash: PLUGIN_CODE_HASH,
                abi: {...WALLET_ABI_RECORD.compilerAbi, contract_name: "Recovered plugin"},
              },
            ],
          },
        })
      }
    },
  )
  return {
    pluginTypeRequests,
    pluginAbiRequests,
    allowPluginTypes: () => {
      pluginTypesFail = false
      missingPluginStates = []
    },
    allowPluginAbi: () => {
      pluginAbi = "available"
    },
  }
}

test.describe("Wallet V5 plugins", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "explorer"})
  })

  test("opens Plugins beside Contract and Methods while leaving the default header compact", async ({
    page,
  }, testInfo) => {
    await mockWallet(page, {codeHash: WALLET_V5_CODE_HASH, withWalletAbi: true})
    await page.goto(`/address/${WALLET_ADDRESS}?network=testnet`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    const pluginsTab = page.getByRole("button", {name: "Plugins", exact: true})
    const methodsTab = page.getByRole("button", {name: "Methods", exact: true})
    await expect(pluginsTab).toBeVisible()
    await expect(methodsTab).toBeVisible()
    await expect(plugins).toHaveCount(0)
    await expect(page.getByText("Signature auth", {exact: true})).toHaveCount(0)
    await expect(page.getByText("No actions found", {exact: true})).toBeVisible()
    await expect(pluginsTab.locator("..").getByRole("button")).toHaveText([
      "History",
      "Tokens",
      "Contract",
      "Methods",
      "Plugins",
    ])

    await pluginsTab.click()
    await expect(page).toHaveURL(`/address/${WALLET_ADDRESS}?network=testnet#plugins`)
    await expect(plugins.getByText("Signature auth", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    const screenshotPath = testInfo.outputPath("wallet-v5-plugins-tab.png")
    await page.screenshot({path: screenshotPath, fullPage: true})
    await testInfo.attach("wallet-v5-plugins-tab", {
      path: screenshotPath,
      contentType: "image/png",
    })

    await page.reload()
    await expect(page).toHaveURL(`/address/${WALLET_ADDRESS}?network=testnet#plugins`)
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await page.getByRole("button", {name: "History", exact: true}).click()
    await expect(page).toHaveURL(`/address/${WALLET_ADDRESS}?network=testnet#history`)
    await expect(plugins).toHaveCount(0)
    await expect(page.getByText("Signature auth", {exact: true})).toHaveCount(0)
  })

  for (const network of ["mainnet", "testnet"] as const) {
    test(`shows abbreviated plugin addresses and preserves full ${network} addresses when using them`, async ({
      page,
    }, testInfo) => {
      await mockWallet(page)
      await page.goto(`/address/${WALLET_ADDRESS}?network=${network}#plugins`)

      const plugins = page.getByRole("region", {name: "Plugins", exact: true})
      await expect(plugins).toBeVisible()
      const table = plugins.getByRole("table", {name: "Installed plugins", exact: true})
      await expect(table.getByRole("columnheader")).toHaveText(["Address", "Type"])
      await expect(table.locator("tbody > tr")).toHaveCount(2)
      await expect(plugins.getByRole("heading", {name: "Plugins", exact: true})).toHaveCount(0)
      const signatureAuth = plugins.getByText("Signature auth", {exact: true})
      await expect(signatureAuth).toHaveCount(1)
      await expect(signatureAuth).toBeVisible()
      const signatureBounds = await signatureAuth.boundingBox()
      const tableBounds = await table.boundingBox()
      expect(signatureBounds).not.toBeNull()
      expect(tableBounds).not.toBeNull()
      expect((signatureBounds?.y ?? 0) + (signatureBounds?.height ?? 0)).toBeLessThanOrEqual(
        tableBounds?.y ?? 0,
      )
      await expect(plugins.getByText("Enabled", {exact: true})).toBeVisible()
      await expect(plugins.getByText("Disabled", {exact: true})).toHaveCount(0)
      await expect(plugins.getByRole("button", {name: "Copy address", exact: true})).toHaveCount(2)
      await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
      await expect(plugins.getByText("Unknown", {exact: true})).toBeVisible()
      await expect(plugins.getByText("Wallet V5", {exact: true})).toHaveCount(0)
      await expect(
        plugins.getByText("Contracts authorized to send transactions on behalf of this wallet."),
      ).toHaveCount(0)

      const addresses = PLUGIN_ADDRESSES.map(address =>
        Address.parse(address).toString({testOnly: network === "testnet", bounceable: true}),
      )
      for (const address of addresses) {
        await expect(
          plugins.getByRole("button", {name: abbreviatedAddress(address), exact: true}),
        ).toBeVisible()
      }
      const screenshotPath = testInfo.outputPath(`wallet-v5-${network}.png`)
      await plugins.screenshot({path: screenshotPath})
      await testInfo.attach(`wallet-v5-${network}`, {
        path: screenshotPath,
        contentType: "image/png",
      })

      const firstAddress = plugins.getByRole("button", {
        name: abbreviatedAddress(addresses[0]),
        exact: true,
      })
      // Capture the copy operation without changing the user's system clipboard.
      await page.evaluate(() => {
        navigator.clipboard.writeText = value => {
          sessionStorage.setItem("wallet-v5-test-copied-address", value)
          return Promise.resolve()
        }
      })
      await firstAddress.hover()
      await table
        .getByRole("row")
        .filter({hasText: abbreviatedAddress(addresses[0])})
        .getByRole("button", {name: "Copy address", exact: true})
        .click()
      await expect
        .poll(() => page.evaluate(() => sessionStorage.getItem("wallet-v5-test-copied-address")))
        .toBe(addresses[0])

      await firstAddress.click()
      await expect(page).toHaveURL(`/address/${addresses[0]}?network=${network}`)
      await expect(plugins).toHaveCount(0)
    })
  }

  test("renders the standard plugin table in dark theme", async ({page}, testInfo) => {
    await mockWallet(page, {codeHash: WALLET_V5_CODE_HASH, withWalletAbi: true})
    await page.goto(`/address/${WALLET_ADDRESS}?network=mainnet#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    const table = plugins.getByRole("table", {name: "Installed plugins", exact: true})
    await expect(table.getByRole("columnheader")).toHaveText(["Address", "Type"])
    await expect(table.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(table.getByText("Unknown", {exact: true})).toBeVisible()
    await page.getByRole("button", {name: "Use dark theme", exact: true}).click()
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
    const screenshotPath = testInfo.outputPath("wallet-v5-plugins-dark.png")
    await page.screenshot({path: screenshotPath, fullPage: true, animations: "disabled"})
    await testInfo.attach("wallet-v5-plugins-dark", {
      path: screenshotPath,
      contentType: "image/png",
    })
  })

  for (const signatureAllowed of [true, false]) {
    test(`shows an empty plugin list from the code hash with stored signature flag ${signatureAllowed}`, async ({
      page,
    }) => {
      await mockWallet(page, {
        data: walletData([], signatureAllowed),
        interfaces: [],
        codeHash: WALLET_V5_CODE_HASH,
      })
      await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

      const plugins = page.getByRole("region", {name: "Plugins", exact: true})
      const table = plugins.getByRole("table", {name: "Installed plugins", exact: true})
      await expect(
        table.getByRole("cell", {name: "No plugins installed", exact: true}),
      ).toBeVisible()
      await expect(table.locator("tbody > tr")).toHaveCount(1)
      await expect(plugins.getByRole("button", {name: "Copy address", exact: true})).toHaveCount(0)
      await expect(
        plugins.getByText(signatureAllowed ? "Enabled" : "Disabled", {exact: true}),
      ).toBeVisible()
      await expect(
        plugins.getByText(signatureAllowed ? "Disabled" : "Enabled", {exact: true}),
      ).toHaveCount(0)
      await expect(plugins.getByText(AUTO_ENABLE_HINT, {exact: true})).toHaveCount(
        signatureAllowed ? 0 : 1,
      )
      if (!signatureAllowed) {
        await plugins.getByRole("button", {name: "About signature authorization"}).click()
        const explanation = page.getByRole("dialog", {name: "About signature authorization"})
        await expect(explanation).toContainText("Signature auth is disabled.")
        await expect(explanation).toContainText(AUTO_ENABLE_EXPLANATION)
        await expect(explanation).not.toContainText("rejected")
      }
    })
  }

  test("shows disabled signature authentication while retaining the installed plugins", async ({
    page,
  }) => {
    await mockWallet(page, {data: walletData(PLUGIN_ADDRESSES, false)})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByText("Disabled", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Enabled", {exact: true})).toHaveCount(0)
    await expect(plugins.getByText(AUTO_ENABLE_HINT, {exact: true})).toHaveCount(0)
    await expect(plugins.getByRole("button", {name: "Copy address", exact: true})).toHaveCount(2)
  })

  test("keeps the automatic signature hint readable on a narrow viewport", async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({width: 360, height: 800})
    await mockWallet(page, {data: walletData([], false)})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    const status = plugins.getByText("Disabled", {exact: true})
    const hint = plugins.getByText(AUTO_ENABLE_HINT, {exact: true})
    await expect(status).toBeVisible()
    await expect(hint).toBeVisible()
    await expect(plugins.getByText("Enabled", {exact: true})).toHaveCount(0)
    const statusBounds = await status.boundingBox()
    const hintBounds = await hint.boundingBox()
    expect(statusBounds).not.toBeNull()
    expect(hintBounds).not.toBeNull()
    expect(hintBounds?.x ?? -1).toBeGreaterThanOrEqual(0)
    expect((hintBounds?.x ?? 0) + (hintBounds?.width ?? 361)).toBeLessThanOrEqual(360)
    expect(hintBounds?.y ?? 0).toBeGreaterThanOrEqual(
      (statusBounds?.y ?? 0) + (statusBounds?.height ?? 0),
    )
    const screenshotPath = testInfo.outputPath("wallet-v5-auto-signature-mobile.png")
    await page.screenshot({path: screenshotPath, fullPage: true})
    await testInfo.attach("wallet-v5-auto-signature-mobile", {
      path: screenshotPath,
      contentType: "image/png",
    })
  })

  test("keeps plugin addresses visible while resolving their types", async ({page}) => {
    const pluginTypes = Promise.withResolvers<void>()
    await mockWallet(page, {pluginTypesReady: pluginTypes.promise})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(2)
    await expect(plugins.getByRole("button", {name: "Copy address", exact: true})).toHaveCount(2)
    pluginTypes.resolve()
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Unknown", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(0)
  })

  test("keeps skeletons after a temporary account failure and recovers automatically after a delay", async ({
    page,
  }, testInfo) => {
    await page.clock.install()
    const fixture = await mockWallet(page, {pluginTypesFail: true})
    const failedResponse = page.waitForResponse(
      response => response.url().includes("/accountStates") && response.status() === 503,
    )
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)
    await (await failedResponse).finished()

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(2)
    await expect(plugins.getByText("Type unavailable", {exact: true})).toHaveCount(0)
    await expect(plugins.getByRole("button", {name: "Retry types", exact: true})).toHaveCount(0)
    await expect(plugins.getByText("Unknown", {exact: true})).toHaveCount(0)
    await expect(plugins.getByRole("button", {name: "Copy address", exact: true})).toHaveCount(2)
    await expect(plugins.getByText("Enabled", {exact: true})).toBeVisible()
    await page.clock.fastForward(1000)
    expect(fixture.pluginTypeRequests).toHaveLength(1)
    const screenshotPath = testInfo.outputPath("wallet-v5-loading-types.png")
    await plugins.screenshot({path: screenshotPath})
    await testInfo.attach("wallet-v5-loading-types", {
      path: screenshotPath,
      contentType: "image/png",
    })
    fixture.allowPluginTypes()
    await page.clock.fastForward(5000)
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Unknown", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(0)
    expect(fixture.pluginTypeRequests).toHaveLength(2)
    await page.clock.fastForward(15_000)
    expect(fixture.pluginTypeRequests).toHaveLength(2)
  })

  test("retains successful types while automatically retrying only missing plugin states", async ({
    page,
  }) => {
    await page.clock.install()
    const fixture = await mockWallet(page, {missingPluginStates: [PLUGIN_ADDRESSES[1]]})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(1)
    await page.clock.fastForward(6000)
    await expect.poll(() => fixture.pluginTypeRequests).toHaveLength(2)
    expect(fixture.pluginTypeRequests[1]).toEqual([PLUGIN_ADDRESSES[1]])
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(1)

    fixture.allowPluginTypes()
    await page.clock.fastForward(6000)
    await expect(plugins.getByText("Unknown", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(0)
    expect(fixture.pluginTypeRequests[2]).toEqual([PLUGIN_ADDRESSES[1]])
  })

  test("keeps an unresolved ABI loading through verifier failures while retaining other types", async ({
    page,
  }) => {
    await page.clock.install()
    const fixture = await mockWallet(page, {pluginAbi: "unavailable"})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(1)
    await expect(plugins.getByText("Unknown", {exact: true})).toHaveCount(0)
    await expect(plugins.getByText("Type unavailable", {exact: true})).toHaveCount(0)
    await expect(plugins.getByRole("button", {name: "Retry types", exact: true})).toHaveCount(0)
    expect(fixture.pluginAbiRequests).toHaveLength(1)

    fixture.allowPluginAbi()
    await page.clock.fastForward(6000)
    await expect(plugins.getByText("Recovered plugin", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(0)
    expect(fixture.pluginTypeRequests[1]).toEqual([PLUGIN_ADDRESSES[1]])
    expect(fixture.pluginAbiRequests).toHaveLength(2)
    await page.clock.fastForward(15_000)
    expect(fixture.pluginTypeRequests).toHaveLength(2)
    expect(fixture.pluginAbiRequests).toHaveLength(2)
  })

  test("treats a missing verifier ABI as a terminal unknown type without polling", async ({
    page,
  }) => {
    await page.clock.install()
    const fixture = await mockWallet(page, {pluginAbi: "missing"})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByText("Unknown", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(0)
    expect(fixture.pluginAbiRequests).toHaveLength(1)
    await page.clock.fastForward(15_000)
    expect(fixture.pluginTypeRequests).toHaveLength(1)
    expect(fixture.pluginAbiRequests).toHaveLength(1)
  })

  for (const destination of ["history", "another account"] as const) {
    test(`cancels pending plugin retries when navigating to ${destination}`, async ({page}) => {
      await page.clock.install()
      const fixture = await mockWallet(page, {pluginTypesFail: true})
      const failedResponse = page.waitForResponse(
        response => response.url().includes("/accountStates") && response.status() === 503,
      )
      await page.goto(`/address/${WALLET_ADDRESS}?network=mainnet#plugins`)
      await (await failedResponse).finished()
      const plugins = page.getByRole("region", {name: "Plugins", exact: true})
      await expect(plugins.getByLabel("Loading plugin type")).toHaveCount(2)

      if (destination === "history") {
        await page.getByRole("button", {name: "History", exact: true}).click()
        await expect(page).toHaveURL(`/address/${WALLET_ADDRESS}?network=mainnet#history`)
      } else {
        const address = Address.parse(PLUGIN_ADDRESSES[0]).toString({bounceable: true})
        await plugins.getByRole("button", {name: abbreviatedAddress(address), exact: true}).click()
        await expect(page).toHaveURL(`/address/${address}?network=mainnet`)
      }
      await expect(plugins).toHaveCount(0)
      const requestsAfterNavigation = fixture.pluginTypeRequests.length
      await page.clock.fastForward(15_000)
      expect(fixture.pluginTypeRequests).toHaveLength(requestsAfterNavigation)
    })
  }

  test("opens signature help on hover and click and dismisses it with Escape", async ({page}) => {
    await mockWallet(page)
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins.getByText("Enabled", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Disabled", {exact: true})).toHaveCount(0)
    await expect(plugins.getByText(AUTO_ENABLE_HINT, {exact: true})).toHaveCount(0)
    const help = page.getByRole("button", {name: "About signature authorization", exact: true})
    const explanation = page.getByRole("dialog", {name: "About signature authorization"})
    await help.hover()
    await expect(explanation).toBeVisible()
    await expect(explanation).toContainText("Signature auth is enabled.")
    await page.mouse.move(0, 0)
    await expect(explanation).toBeHidden()
    await help.click()
    await expect(explanation).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(explanation).toBeHidden()
  })

  test("opens signature help with Enter and Space from keyboard focus", async ({page}) => {
    await mockWallet(page, {data: walletData(PLUGIN_ADDRESSES, false)})
    await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

    const help = page.getByRole("button", {name: "About signature authorization", exact: true})
    const explanation = page.getByRole("dialog", {name: "About signature authorization"})
    await help.focus()
    await page.keyboard.press("Enter")
    await expect(explanation).toBeVisible()
    await expect(explanation).toContainText("Signature auth is disabled.")
    await expect(explanation).toContainText(
      "Transactions signed with the owner's key are rejected.",
    )
    await page.keyboard.press("Escape")
    await expect(explanation).toBeHidden()
    await expect(help).toBeFocused()
    await page.keyboard.press("Space")
    await expect(explanation).toBeVisible()
  })

  test.describe("touchscreen", () => {
    test.use({hasTouch: true, isMobile: true, viewport: {width: 360, height: 800}})

    test("opens signature help by tapping and keeps the popup within the viewport", async ({
      page,
    }, testInfo) => {
      await mockWallet(page)
      await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

      const help = page.getByRole("button", {name: "About signature authorization", exact: true})
      const explanation = page.getByRole("dialog", {name: "About signature authorization"})
      await help.tap()
      await expect(explanation).toBeVisible()
      await expect(explanation).toContainText("Signature auth is enabled.")
      const bounds = await explanation.boundingBox()
      expect(bounds).not.toBeNull()
      expect(bounds?.x ?? -1).toBeGreaterThanOrEqual(0)
      expect((bounds?.x ?? 0) + (bounds?.width ?? 361)).toBeLessThanOrEqual(360)
      const screenshotPath = testInfo.outputPath("wallet-v5-signature-help-mobile.png")
      await page.screenshot({path: screenshotPath, fullPage: true})
      await testInfo.attach("wallet-v5-signature-help-mobile", {
        path: screenshotPath,
        contentType: "image/png",
      })
      await page.getByRole("columnheader", {name: "Type", exact: true}).tap()
      await expect(explanation).toBeHidden()
    })
  })

  for (const [name, data] of [
    ["missing", null],
    ["malformed", "not-a-boc"],
  ] as const) {
    test(`reports unavailable plugins for ${name} storage without claiming an empty list`, async ({
      page,
    }) => {
      await mockWallet(page, {data})
      await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

      const plugins = page.getByRole("region", {name: "Plugins", exact: true})
      await expect(plugins.getByText("Plugin data is unavailable", {exact: true})).toBeVisible()
      await expect(plugins.getByText("No plugins installed", {exact: true})).toHaveCount(0)
      await expect(plugins.getByText("Enabled", {exact: true})).toHaveCount(0)
      await expect(plugins.getByText("Disabled", {exact: true})).toHaveCount(0)
    })
  }

  for (const options of [
    {status: "frozen", interfaces: ["wallet_v5r1"]},
    {status: "uninitialized", interfaces: ["wallet_v5r1"]},
    {status: "active", interfaces: ["wallet_v5_beta"]},
    {status: "active", interfaces: ["wallet_v4r2"]},
  ] as const) {
    test(`omits plugins for ${options.status} ${options.interfaces[0]}`, async ({page}) => {
      await mockWallet(page, options)
      await page.goto(`/address/${WALLET_ADDRESS}#plugins`)

      await expect(page.getByText("Balance", {exact: true})).toBeVisible()
      await expect(page.getByRole("region", {name: "Plugins", exact: true})).toHaveCount(0)
      await expect(page.getByRole("button", {name: "Plugins", exact: true})).toHaveCount(0)
    })
  }

  test("keeps abbreviated plugin addresses and copy controls within a narrow viewport", async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({width: 360, height: 800})
    await mockWallet(page)
    await page.goto(`/address/${WALLET_ADDRESS}?network=testnet#plugins`)

    const plugins = page.getByRole("region", {name: "Plugins", exact: true})
    await expect(plugins).toBeVisible()
    await expect(page.getByRole("button", {name: "Plugins", exact: true})).toBeInViewport({
      ratio: 1,
    })
    await expect(plugins.getByText("Multisig wallet v2", {exact: true})).toBeVisible()
    await expect(plugins.getByText("Unknown", {exact: true})).toBeVisible()
    await expect(plugins.getByRole("button", {name: "Copy address", exact: true})).toHaveCount(2)
    for (const address of PLUGIN_ADDRESSES) {
      const friendly = Address.parse(address).toString({testOnly: true, bounceable: true})
      await expect(
        plugins.getByRole("button", {name: abbreviatedAddress(friendly), exact: true}),
      ).toBeVisible()
    }
    for (const control of await plugins.getByRole("button").all()) {
      const bounds = await control.boundingBox()
      expect(bounds).not.toBeNull()
      expect(bounds?.x ?? -1).toBeGreaterThanOrEqual(0)
      expect((bounds?.x ?? 0) + (bounds?.width ?? 361)).toBeLessThanOrEqual(360)
    }
    const screenshotPath = testInfo.outputPath("wallet-v5-mobile.png")
    await plugins.screenshot({path: screenshotPath})
    await testInfo.attach("wallet-v5-mobile", {
      path: screenshotPath,
      contentType: "image/png",
    })
  })
})
