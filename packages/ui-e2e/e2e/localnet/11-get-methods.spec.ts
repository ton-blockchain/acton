import {expect, test, type Page} from "@playwright/test"

import {JETTON_MASTER_ADDRESS, mockJettonMaster} from "../support/jettonMaster"
import {expectVisualSnapshot, prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

const ACCOUNT_CODE_HASH = "20834b7b72b112147e1b2fb457b84e74d1a30f04f737d4f62a668e9552d2b72f"

const compilerAbi = {
  abi_schema_version: "1.0",
  alias_instantiations: [],
  compiler_name: "tolk",
  compiler_version: "1.4.2",
  contract_name: "VisualToken",
  declarations: [],
  emitted_events: [],
  get_methods: [
    {
      name: "balanceOf",
      description: "Returns the token balance available to an owner",
      parameters: [
        {name: "owner", ty_idx: 7},
        {name: "includeLocked", ty_idx: 5},
      ],
      return_ty_idx: 6,
      tvm_method_id: 107_486,
    },
    {
      name: "totalSupply",
      description: "Returns the total minted supply",
      parameters: [],
      return_ty_idx: 6,
      tvm_method_id: 90_601,
    },
  ],
  incoming_external: [],
  incoming_messages: [],
  outgoing_messages: [],
  storage: {},
  struct_instantiations: [],
  thrown_errors: [
    {
      err_code: 292,
      name: "InvalidAsset",
      description: "The provided asset cannot be used by this method",
    },
  ],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
  ],
}

const mockCompilerAbi = async (page: Page) => {
  await page.route(
    url => url.pathname === "/acton_getCompilerAbi",
    async route =>
      route.fulfill({
        json: {
          [ACCOUNT_CODE_HASH]: {
            compiler_abi: compilerAbi,
            code_hashes: [ACCOUNT_CODE_HASH],
            links: [],
          },
        },
      }),
  )
}

const openGetMethods = async (page: Page) => {
  await prepareVisualPage(page, {app: "localnet"})
  await mockJettonMaster(page, true)
  await mockCompilerAbi(page)
  await page.goto(`/explorer/address/${JETTON_MASTER_ADDRESS}#get-methods`)
  await expect(page.getByRole("button", {name: "Methods"})).toBeVisible()
  await expect(page.locator("article").filter({hasText: "balanceOf"})).toBeVisible()
}

const mockGetMethodError = async (page: Page) => {
  await page.route(
    url => url.pathname === "/api/v3/runGetMethod",
    async route => {
      const request = route.request().postDataJSON() as {readonly method: string}
      expect(request.method).toBe("totalSupply")
      await route.fulfill({
        json: {
          exit_code: 292,
          gas_used: 143,
          stack: [],
          vm_log: "",
        },
      })
    },
  )
}

test.describe("Account get methods", () => {
  test("shows network-aware address formats on hover", async ({page}) => {
    await openGetMethods(page)

    await page.getByLabel("Show address formats").hover()
    const popover = page.getByRole("dialog", {name: "Address formats", exact: true})
    const values = popover.locator("code")

    await expect(popover).toBeVisible()
    await expect(values).toHaveCount(3)
    await expect(values.nth(0)).toHaveText(/^k/)
    await expect(values.nth(1)).toHaveText(/^0/)
    await expect(values.nth(2)).toHaveText(JETTON_MASTER_ADDRESS)

    await expect(popover.getByRole("button", {name: /^Copy .* address$/})).toHaveCount(3)
  })

  test("opens ABI get methods in a dedicated account tab and runs one", async ({page}) => {
    await page.route(
      url => url.pathname === "/api/v3/runGetMethod",
      async route => {
        const request = route.request().postDataJSON() as {
          readonly method: string
          readonly stack: readonly unknown[]
        }
        expect(request.method).toBe("balanceOf")
        expect(request.stack).toHaveLength(2)
        await route.fulfill({
          json: {
            exit_code: 0,
            gas_used: 143,
            stack: [{type: "num", value: "1000000000"}],
            vm_log: "",
          },
        })
      },
    )
    await openGetMethods(page)

    const method = page.locator("article").filter({hasText: "balanceOf"})
    const includeLocked = method.getByRole("checkbox")
    await method.getByRole("combobox", {name: "owner"}).fill(JETTON_MASTER_ADDRESS)
    await includeLocked.check()
    await method.getByRole("button", {name: "Run"}).click()

    const decodedResult = method.getByText("1000000000", {exact: true})
    await expect(decodedResult).toBeVisible()
    await includeLocked.uncheck()
    await expect(decodedResult).toBeHidden()
    await expect(page).toHaveURL(/#get-methods$/)
  })

  test("links a resolved get-method error to its ABI declaration", async ({page}) => {
    await mockGetMethodError(page)
    await openGetMethods(page)

    const method = page.locator("article").filter({hasText: "totalSupply"})
    await method.getByRole("button", {name: "Run"}).click()
    await expect(method).toContainText("Get method exited with InvalidAsset (292).")

    await method.getByRole("link", {name: "InvalidAsset"}).click()
    await expect(page).toHaveURL(/#abi-error-invalidasset-292$/)
    await expect(page.getByText("InvalidAsset", {exact: true})).toBeVisible()
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    test("loc-account-get-methods", async ({page}) => {
      await openGetMethods(page)
      await expectVisualSnapshot(page, "loc-account-get-methods")
    })

    test("loc-account-address-formats", async ({page}) => {
      await openGetMethods(page)
      await page.getByLabel("Show address formats").hover()
      await expect(page.getByRole("dialog", {name: "Address formats", exact: true})).toBeVisible()
      await expectVisualSnapshot(page, "loc-account-address-formats")
    })

    test("loc-account-get-method-error", async ({page}) => {
      await mockGetMethodError(page)
      await openGetMethods(page)

      const method = page.locator("article").filter({hasText: "totalSupply"})
      await method.getByRole("button", {name: "Run"}).click()
      await expect(method.getByRole("link", {name: "InvalidAsset"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-account-get-method-error")
    })
  })
})
