import {expect, test, type Page} from "@playwright/test"

import abi from "../../../transaction-ui/test/fixtures/abiRendering.json" with {type: "json"}
import {prepareVisualPage} from "../support/visual"

// ABI comes from transaction-ui/test/fixtures/abiRendering.tolk.
function section(page: Page, name: string) {
  return page.getByRole("heading", {name, exact: true}).locator("../..")
}

test.describe("ABI rendering", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "explorer"})
    await page.route(
      url =>
        url.pathname.includes("data-abis") &&
        url.pathname.endsWith(".json") &&
        !url.searchParams.has("url"),
      route =>
        route.fulfill({
          json: {
            schemaVersion: 1,
            contracts: [
              {
                id: "test.AbiRendering",
                displayName: "ABI rendering",
                hashes: [],
                knownAddresses: [],
                compilerAbi: abi,
              },
            ],
          },
        }),
    )
    await page.goto("/abi/abi-rendering")
    await expect(page.getByRole("heading", {name: "AbiRendering", exact: true})).toBeVisible()
  })

  test("renders concrete storage and messages with distinct generic anchors", async ({page}) => {
    await expect(section(page, "Storage").locator("code")).toHaveText([
      "struct Storage<uint32> { value: uint32 history: Cell<Storage<uint32>>? status: Status enabled: bool counter: uint64 }",
      "struct Storage<address> { value: address history: Cell<Storage<address>>? status: Status enabled: bool counter: uint64 }",
    ])
    await expect(section(page, "Messages").locator("code")).toHaveText([
      "struct GenericMessage<uint32> { value: uint32 }",
      "struct GenericMessage<address> { value: address }",
      "struct (0b001) Message<address> { /// The message payload value: address }",
      "type Wrapped<address> = Message<address>",
      "struct (0b001) Message<bool> { /// The message payload value: bool }",
      "struct (0x123456789abc) WidePrefix { value: uint8 }",
    ])
    await page.getByRole("link", {name: "Link to GenericMessage<address>", exact: true}).click()
    await expect(
      page.locator("#abi-message-incoming-internal-genericmessage-address"),
    ).toBeInViewport()
  })

  test("renders all template declarations and their serialization metadata", async ({page}) => {
    const declarations = section(page, "Declarations")
    for (const summary of await declarations.locator("summary").all()) {
      await summary.click()
    }
    await expect(declarations.locator("summary")).toContainText([
      "Storage<T>",
      "Message<T>",
      "Empty<T>",
      "GenericMessage<T>",
      "Pair<L, R>",
      "Either<L, R>",
      "WidePrefix",
      "Wrapped<T>",
      "Status",
      "Meta",
      "`odd-name`",
      "`type`",
      "`odd-alias`<T>",
      "WirePayload",
      "ClientMessage",
      "CustomInt",
      "Defaults",
    ])
    await expect(page.locator("#abi-declaration-pair code")).toHaveText(
      "struct Pair<L, R> { left: L right: R }",
    )
    await expect(page.locator("#abi-declaration-either code")).toHaveText(
      "type Either<L, R> = L | R",
    )
    await expect(page.locator("#abi-declaration-status code")).toHaveText(
      "enum Status: uint8 { /// Ready to receive messages Ready = 0 Busy = 7 }",
    )
    await expect(page.locator("#abi-declaration-type code")).toHaveText(
      "struct `type` { `fun`: uint8 }",
    )
    await expect(page.locator("#abi-declaration-clientmessage code")).toHaveText(
      "struct ClientMessage { @abi.clientType(Cell<Message<uint32>>) payload: WirePayload }",
    )
    await expect(page.locator("#abi-declaration-customint code")).toHaveText(
      "// Custom serialization: packToBuilder, unpackFromSlice type CustomInt = int32",
    )
    const defaults = page.locator("#abi-declaration-defaults code")
    for (const value of [
      "count: int = 0",
      "large: uint64 = 18446744073709551615",
      "enabled: bool = false",
      'text: string = "hello\\n\\"world\\""',
      'bits: slice = "ABCD".hexToSlice()',
      'owner: address = address("0:0000000000000000000000000000000000000000000000000000000000000000")',
      "pair: (int, bool) = (1, true)",
      "tupleValue: [int, bool] = ([2, false] as [int, bool])",
      "meta: Meta = Meta { value: (7 as uint8) }",
      "message: Message<uint32> = Message<uint32> { value: (1 as uint32) }",
      "wrapped: Wrapped<uint32> = Message<uint32> { value: (2 as uint32) }",
      "nested: (Message<uint32>, Meta) = (Message<uint32> { value: (3 as uint32) }, Meta { value: (4 as uint8) })",
      "boxed: [Message<uint32>] = ([Message<uint32> { value: (5 as uint32) }] as [Message<uint32>])",
      "items: array<Message<uint32>> = ([Message<uint32> { value: (6 as uint32) }] as array<Message<uint32>>)",
      "state: Status = (7 as Status)",
      "optional: int? = null",
      "custom: CustomInt = (42 as CustomInt)",
    ]) {
      await expect(defaults).toContainText(value)
    }
  })

  test("renders getter defaults and opens concrete struct and alias definitions", async ({
    page,
  }) => {
    await expect(page.locator("#abi-get-method-get-storage code")).toHaveText(
      "get fun get_storage(limit: int = 10): Storage<uint32>",
    )
    await expect(page.locator("#abi-get-method-get-defaults code")).toContainText(
      "value: Defaults = Defaults {",
    )
    await expect(page.locator("#abi-get-method-return code")).toHaveText(
      "get fun `return`(`val`: `type`): `type`",
    )
    await expect(page.locator("#abi-get-method-get-types code")).toContainText(
      "items: array<`odd-name`>",
    )
    await page.getByRole("button", {name: "Storage<uint32>", exact: true}).click()
    await expect(page.getByRole("dialog").locator("code")).toHaveText(
      "struct Storage<uint32> { value: uint32 history: Cell<Storage<uint32>>? status: Status enabled: bool counter: uint64 }",
    )
    await page.keyboard.press("Escape")
    await page.getByRole("button", {name: "Wrapped<address>", exact: true}).click()
    await expect(page.getByRole("dialog").locator("code")).toHaveText(
      "type Wrapped<address> = Message<address>",
    )
    await page.keyboard.press("Escape")
    await page
      .getByRole("button", {name: "Pair<uint32, Wrapped<address>>", exact: true})
      .first()
      .click()
    await expect(page.getByRole("dialog").locator("code")).toHaveText(
      "struct Pair<uint32, Wrapped<address>> { left: uint32 right: Wrapped<address> }",
    )
    await page.keyboard.press("Escape")
    await page.getByRole("button", {name: "Storage<uint32>?", exact: true}).first().click()
    await expect(page.getByRole("dialog").locator("code")).toHaveText(
      "Storage<uint32>? struct Storage<uint32> { value: uint32 history: Cell<Storage<uint32>>? status: Status enabled: bool counter: uint64 }",
    )
  })
})
