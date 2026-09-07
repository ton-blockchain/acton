// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test, type Page} from "@playwright/test"
import {Address} from "@ton/core"
import type {LocalnetContract} from "@acton/explorer-core/api/types"
import type {StudioEnvironment, StudioWallet} from "../src/studioApi"

const contractAddress = `0:${"12".repeat(32)}`
const walletAddress = `0:${"34".repeat(32)}`
const transactionHash = "ab".repeat(32)
const friendlyContractAddress = Address.parse(contractAddress).toString({testOnly: true})

const development: StudioEnvironment = {
  id: "development",
  name: "Development",
  status: "running",
  lifecycle: "managed",
  rpcUrl: "/rpc/development",
  config: {
    kind: "actonSimulatedLocalnet",
    port: 8081,
    accounts: [],
    noMining: false,
    mineEmptyBlocks: false,
  },
  capabilities: ["explorer", "contracts", "wallets", "snapshots"],
  endpoints: {
    apiV2: "/rpc/development/api/v2",
    apiV3: "/rpc/development/api/v3",
    control: "/rpc/development",
  },
  network: {id: "local", label: "Local", chainId: -3, testOnly: true, supportsActions: false},
}

const mainnet: StudioEnvironment = {
  ...development,
  id: "mainnet",
  name: "Mainnet",
  lifecycle: "external",
  rpcUrl: "/rpc/mainnet",
  endpoints: {apiV2: "/rpc/mainnet/api/v2", apiV3: "/rpc/mainnet/api/v3", control: "/rpc/mainnet"},
  capabilities: ["explorer"],
  network: {id: "mainnet", label: "Mainnet", chainId: -239, testOnly: false, supportsActions: true},
}

interface SearchFixture {
  environments: StudioEnvironment[]
  contracts: LocalnetContract[]
  contractError: boolean
  contractRequests: number
  foreignContractRequests: number
  walletRequests: number
  mutations: string[]
}

async function mockStudio(page: Page): Promise<SearchFixture> {
  const fixture: SearchFixture = {
    environments: [development, mainnet],
    contracts: [
      {
        address: contractAddress,
        name: "Treasury",
        status: "active",
        codeHash: "00".repeat(32),
        sourceKind: "local",
      },
    ],
    contractError: false,
    contractRequests: 0,
    foreignContractRequests: 0,
    walletRequests: 0,
    mutations: [],
  }
  const wallets: StudioWallet[] = [
    {
      name: "deployer",
      address: walletAddress,
      publicKey: "00".repeat(32),
      version: "v4r2",
      walletId: 1,
      workchain: 0,
    },
  ]

  await page.route("**/api/v1/**", async route => {
    const url = new URL(route.request().url())
    if (route.request().method() !== "GET") fixture.mutations.push(url.pathname)
    let body: unknown = []
    if (url.pathname.endsWith("/info")) body = {protocolVersion: 1, serverVersion: "test"}
    if (url.pathname.endsWith("/environments")) body = fixture.environments
    if (url.pathname.endsWith("/wallets")) {
      fixture.walletRequests++
      body = wallets
    }
    if (url.pathname.endsWith("/admin")) body = null
    await route.fulfill({json: body})
  })
  await page.route("**/rpc/**", async route => {
    const url = new URL(route.request().url())
    let body: unknown = {ok: true, result: {}}
    if (url.pathname.endsWith("/acton_listContracts")) {
      if (url.pathname.startsWith("/rpc/development/")) fixture.contractRequests++
      else fixture.foreignContractRequests++
      if (fixture.contractError) {
        await route.fulfill({status: 503, json: {error: "Registry is restarting"}})
        return
      }
      body = fixture.contracts
    }
    if (url.pathname.endsWith("/acton_getContract")) body = fixture.contracts[0]
    await route.fulfill({json: body})
  })
  return fixture
}

async function openSearch(page: Page) {
  await page.getByRole("button", {name: "Search Studio", exact: true}).click()
  return page.getByRole("dialog", {name: "Search Studio", exact: true})
}

test("global search finds named environments and typed pages with keyboard navigation", async ({
  page,
}) => {
  const fixture = await mockStudio(page)
  await page.goto("/")
  const dialog = await openSearch(page)
  const input = dialog.getByRole("combobox", {name: "Search Studio"})
  await input.fill("mainnet")
  await expect(dialog.getByRole("option").first()).toContainText("Mainnet")
  await input.press("Escape")
  await expect(dialog).not.toBeVisible()
  await expect(page.getByRole("button", {name: "Search Studio", exact: true})).toBeFocused()

  await page.keyboard.press("ControlOrMeta+k")
  await expect(input).toBeFocused()
  await input.fill("explorer")
  await expect(dialog.getByRole("option")).toHaveCount(6)
  await expect(dialog.getByRole("option").first()).toContainText("Explorer")
  await input.fill("tests")
  await input.press("Enter")
  await expect(page).toHaveURL(/\/tests$/)
  expect({
    contracts: fixture.contractRequests,
    foreign: fixture.foreignContractRequests,
    wallets: fixture.walletRequests,
    mutations: fixture.mutations,
  }).toEqual({contracts: 0, foreign: 0, wallets: 0, mutations: []})
})

test("current registry names, normalized addresses and wallets open their existing routes", async ({
  page,
}) => {
  await mockStudio(page)
  await page.goto("/virtual-environments/development/contracts")
  const dialog = await openSearch(page)
  const input = dialog.getByRole("combobox", {name: "Search Studio"})
  await input.fill("treasury")
  await expect(dialog.getByRole("listbox")).toMatchAriaSnapshot(`
    - listbox:
      - text: Objects
      - option /Treasury Contract .* Development/
  `)
  await input.fill(friendlyContractAddress)
  await expect(dialog.getByRole("option").first()).toContainText("Treasury")
  await input.press("Enter")
  await expect(page).toHaveURL(new RegExp(`/contracts/${friendlyContractAddress}$`))

  await openSearch(page)
  await input.fill("deployer")
  await expect(dialog.getByRole("option")).toContainText("Wallet")
  await input.press("Enter")
  const address = Address.parse(walletAddress).toString({testOnly: true})
  await expect(page).toHaveURL(new RegExp(`/explorer/address/${address}$`))
})

test("an identifier outside an environment requires an explicit network choice", async ({page}) => {
  await mockStudio(page)
  await page.goto("/")
  const dialog = await openSearch(page)
  const input = dialog.getByRole("combobox", {name: "Search Studio"})
  await input.fill(contractAddress)
  await expect(dialog.getByRole("listbox")).toMatchAriaSnapshot(`
    - listbox:
      - text: Choose an environment
      - option /Open address in Development/
      - option /Open address in Mainnet/
  `)
  await input.press("ArrowDown")
  await input.press("Enter")
  const address = Address.parse(contractAddress).toString({testOnly: false})
  await expect(page).toHaveURL(new RegExp(`/networks/mainnet/explorer/address/${address}$`))
})

test("transaction hashes resolve only in the current network and history keeps that network", async ({
  page,
}) => {
  await mockStudio(page)
  await page.goto("/networks/mainnet/explorer")
  const dialog = await openSearch(page)
  await dialog.getByRole("combobox").fill(transactionHash)
  await expect(dialog.getByRole("option")).toHaveCount(1)
  await dialog.getByRole("combobox").press("Enter")
  await expect(page).toHaveURL(`/networks/mainnet/explorer/tx/${transactionHash}`)
  await page.reload()
  await openSearch(page)
  await expect(dialog.getByRole("option").first()).toContainText("Transaction")
  await expect(dialog.getByRole("option").first()).toContainText("Mainnet")
  await dialog.getByRole("button", {name: /Remove .* from recent/}).click()
  await expect(dialog.getByRole("option").filter({hasText: "Transaction"})).toHaveCount(0)

  await page.goto("/virtual-environments/development/contracts")
  await openSearch(page)
  await expect(dialog.getByRole("option").filter({hasText: "Mainnet"})).toHaveCount(0)
})

test("open search refreshes after an import or restore and closing cancels its polling", async ({
  page,
}) => {
  const fixture = await mockStudio(page)
  await page.goto("/virtual-environments/development/snapshots")
  const dialog = await openSearch(page)
  const input = dialog.getByRole("combobox")
  await input.fill("treasury")
  await expect(dialog.getByRole("option")).toHaveCount(1)
  fixture.contracts = [{...fixture.contracts[0], name: "Imported treasury"}]
  await expect(dialog.getByRole("option")).toContainText("Imported treasury", {timeout: 9000})
  fixture.contracts = []
  await expect(dialog.getByRole("option")).toHaveCount(0, {timeout: 9000})
  await expect(dialog).toContainText("No matches")
  await input.press("Escape")
  const requests = fixture.contractRequests
  await page.waitForTimeout(5500)
  expect(fixture.contractRequests).toBe(requests)
  expect(fixture.foreignContractRequests).toBe(0)
})

test("registry errors appear in toasts while pages and wallets remain searchable", async ({
  page,
}) => {
  const fixture = await mockStudio(page)
  fixture.contractError = true
  await page.goto("/virtual-environments/development/snapshots")
  const dialog = await openSearch(page)
  await expect(page.getByRole("region", {name: "Notifications"})).toContainText(
    "Registry is restarting",
  )
  await expect(dialog).not.toContainText("Registry is restarting")
  await dialog.getByRole("combobox").fill("deployer")
  await expect(dialog.getByRole("option")).toContainText("Wallet")
  fixture.contractError = false
  await dialog.getByRole("button", {name: "Retry", exact: true}).click()
  await dialog.getByRole("combobox").fill("treasury")
  await expect(dialog.getByRole("option")).toContainText("Treasury")
  expect(fixture.mutations).toEqual([])
})

test("stopped environments do not load registries and unsupported pages stay hidden", async ({
  page,
}) => {
  const fixture = await mockStudio(page)
  fixture.environments = [{...development, status: "stopped"}, mainnet]
  await page.goto("/")
  const dialog = await openSearch(page)
  await dialog.getByRole("combobox").fill(contractAddress)
  await expect(dialog.getByRole("option")).toHaveCount(1)
  await expect(dialog.getByRole("option")).toContainText("Mainnet")
  await page.goto("/networks/mainnet/explorer")
  await openSearch(page)
  await dialog.getByRole("combobox").fill("snapshots")
  await expect(dialog.getByRole("option")).toHaveCount(0)
  expect({
    contracts: fixture.contractRequests,
    wallets: fixture.walletRequests,
    mutations: fixture.mutations,
  }).toEqual({contracts: 0, wallets: 0, mutations: []})
})

test("mobile search stays inside the viewport and dismisses navigation after selecting", async ({
  page,
}) => {
  await mockStudio(page)
  await page.setViewportSize({width: 390, height: 844})
  await page.goto("/")
  await page.getByRole("button", {name: "Open navigation menu"}).click()
  const dialog = await openSearch(page)
  await dialog.getByRole("combobox").fill("tests")
  const box = await dialog.boundingBox()
  if (!box) throw new Error("Search dialog has no visible bounds")
  expect(box.x).toBeGreaterThanOrEqual(0)
  expect(box.x + box.width).toBeLessThanOrEqual(390)
  await dialog.getByRole("combobox").press("Enter")
  await expect(page).toHaveURL(/\/tests$/)
  await expect(page.getByRole("button", {name: "Open navigation menu"})).toHaveAttribute(
    "aria-expanded",
    "false",
  )
})

test("recent destinations discard deleted objects and duplicate wallet address routes", async ({
  page,
}) => {
  const fixture = await mockStudio(page)
  fixture.contracts = []
  await page.addInitScript(
    ({walletAddress, contractAddress}) => {
      localStorage.setItem(
        "actonStudioSearchHistory",
        JSON.stringify([
          {
            kind: "wallet",
            title: "Old wallet name",
            value: walletAddress,
            environmentId: "development",
          },
          {
            kind: "address",
            title: "Open address in Explorer",
            value: walletAddress,
            environmentId: "development",
          },
          {
            kind: "contract",
            title: "Removed treasury",
            value: contractAddress,
            environmentId: "development",
          },
          {kind: "environment", title: "Deleted environment", value: "", environmentId: "deleted"},
        ]),
      )
    },
    {walletAddress, contractAddress},
  )
  await page.goto("/virtual-environments/development/snapshots")
  const dialog = await openSearch(page)
  await expect(dialog.getByRole("option").filter({hasText: "deployer"})).toHaveCount(1)
  await expect(
    dialog
      .getByRole("option")
      .filter({hasText: /Old wallet|Removed treasury|Deleted environment|Open address/}),
  ).toHaveCount(0)
  await expect(dialog.getByRole("listbox")).toMatchAriaSnapshot(`
    - listbox:
      - text: Recent
      - option /deployer Wallet .* Development/
      - text: Pages
      - option /Explorer Page .* Development/
      - option /Contracts Page .* Development/
      - option /Wallets Page .* Development/
      - option /Snapshots Page .* Development/
  `)
  await dialog.getByRole("button", {name: "Remove deployer from recent"}).click()
  await expect(dialog.getByRole("option").filter({hasText: /deployer|Open address/})).toHaveCount(0)
})
