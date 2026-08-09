import {expect, test, type Page} from "@playwright/test"

import {prepareVisualPage} from "./visual"

export const CELL_INSPECTOR_FIXTURES = {
  textCommentBase64: "te6ccgEBAQEAHwAAOgAAAABIZWxsbyBmcm9tIENlbGwgSW5zcGVjdG9y",
  textCommentHex:
    "b5ee9c7201010101001f00003a0000000048656c6c6f2066726f6d2043656c6c20496e73706563746f72",
  jettonInternalTransferHex:
    "b5ee9c724101010100570000a9178d45190000000000000000402625a008011ac445debca569067cf73f05b9545361d0dd2c5bad6549bafb73bad27e85c7db00235888bbd794ad20cf9ee7e0b72a8a6c3a1ba58b75aca9375f6e775a4fd0b8fb44054e0b1389",
  multiRootBase64: "te6ccgEBAgIAIQABABwAAAAARmlyc3Qgcm9vdAAeAAAAAFNlY29uZCByb290",
  encryptedCommentBase64: "te6ccgEBAQEADgAAGCFn2ksBAgMEBQYHCA==",
  uint32Base64: "te6cckEBAQEABgAACAAAACoFpvBE",
  abiMessageBase64: "te6cckEBAQEACgAAEAAAAAEAAAAHMYJCUw==",
  tvmCodeHex:
    "B5EE9C7241020A010002B8000114FF00F4A413F4BCF2C80B01020162020903C4D0F8918E34D31F31D72C20BC6A28CC96D33F31FA00308E11D72C23DEECBEF492F23FE1D33F31FA0030E2ED44D0FA0002A0C801FA02CEC9ED54E0D72C20BC6A28CCE302D72C207C53F52CE302D72C22CAF83DE4E302D72C269B90AC6431DC840FF2F003060802E6ED44D001D33FFA00FA50FA50FA0006FA0020FA48FA4830F89221C70591308E3AF892F82A28C8CF8420FA5213FA52C97829541242C8CF83CB04CF85A0CCCCF91684F7B013800B5004D724C8CF8A0040CE12CBF7CF50C705F2E04AE25126A0C801FA02CEC9ED5421935B345BE30D216E915BE30E04050052C8CF91CD8B427226CF0B3F5005FA0213FA5415CEC9C8CF850813FA5201FA0271CF0B6ACCC98011FB000068F8276F10F897A1F82FA07381040282100966018070F837B60972FB02C8CF850812FA528210D53276DBCF0B8ECB3FC9810082FB0001FED33FFA00FA48FA50F401FA0020F404016E913091D1E223FA4430F2D14DF897F89370F83A237271E304F839206E8118B722E304216E811D135803E3045023A825A07381032C70F83CA00170F836A00170F836A07381040282100966018070F837A0BCF2B0ED44D0FA0020FA48FA4830F89222C705F2E0495338BEF2AF5138A10700C0C801FA0212CEC9ED54F82A26C8CF8420FA5213FA52C978C8CF905E3514661ACB3F5008FA02FA5414FA5458FA02CEC9C8CF898801547425C8CF83CB04CF85A0CCCCF91684F7B004800B27D7243615CE12CBF781150DCF0B79CCCCCCC98050FB0000E0F897F839206E81109E58E304718102F270F8380170F836A0810FE770F836A0BCF2B0ED44D0FA0020FA48FA4830F89222C705F2E04904D33FFA00FA50305351BEF2AF5151A1C801FA0214CEC9ED54C8CF91EF765F7ACB3F58FA02FA52FA54C9C8CF858812FA5271CF0B6ECCC98050FB00001DA0F605DA89A1F401F491F49061F055DEA90626",
} as const

const TVM_CODE_HASH = "7bfa53bce90ce26cd368ec2989eba2bd15d286104742f0e04659f485a03012ba"
const REGISTRY_COMPILER_ABI = {
  alias_instantiations: [],
  compiler_name: "tolk",
  compiler_version: "1.4.2",
  contract_name: "Counter",
  declarations: [
    {
      fields: [{name: "value", ty_idx: 0}],
      kind: "struct",
      name: "Increment",
      prefix: {prefix_len: 32, prefix_num: 1},
      ty_idx: 1,
    },
  ],
  emitted_events: [],
  get_methods: [],
  incoming_external: [],
  incoming_messages: [{body_ty_idx: 1}],
  outgoing_messages: [],
  storage: {storage_ty_idx: 1},
  struct_instantiations: [],
  thrown_errors: [],
  unique_types: [
    {kind: "intN", n: 32},
    {kind: "StructRef", struct_name: "Increment"},
  ],
} as const

interface CellInspectorSuiteOptions {
  readonly app: "explorer" | "localnet"
  readonly route: string
}

export function describeCellInspector({app, route}: CellInspectorSuiteOptions): void {
  test.describe("Cell Inspector", () => {
    test.beforeEach(async ({page}) => {
      await prepareVisualPage(page, {app})
      await page.goto(route)
      await expect(page.getByRole("heading", {name: "Cell Inspector", exact: true})).toBeVisible()
    })

    test("parses a built-in text comment", async ({page}) => {
      await expect(page.getByRole("checkbox", {name: /Strict parsing/})).not.toBeChecked()
      await inspect(page, CELL_INSPECTOR_FIXTURES.textCommentBase64)

      await expect(page.getByText("Text comment", {exact: true})).toBeVisible()
      await expect(page.getByText(/TON comment/)).toBeVisible()
      await expect(page.getByText("Hello from Cell Inspector", {exact: true})).toBeVisible()
      await expectOutputTabs(page)
      await expectTvmCodeSecondary(page)
    })

    test("normalizes a hexadecimal BoC", async ({page}) => {
      await inspect(page, CELL_INSPECTOR_FIXTURES.textCommentHex)

      await expect(page.getByText("Text comment", {exact: true})).toBeVisible()
      await expect(page.getByText("Hello from Cell Inspector", {exact: true})).toBeVisible()
    })

    test("prefers an inferred ABI schema over the generic TL-B parser", async ({page}) => {
      await inspect(page, CELL_INSPECTOR_FIXTURES.jettonInternalTransferHex)

      await expect(
        page.getByText("ABI catalog · JettonInternalTransfer", {exact: true}),
      ).toBeVisible()
      await expect(page.getByText(/Contract ABI/)).toBeVisible()
      await expect(page.getByText(/ABI definitions decode/)).toBeVisible()
      await expectTvmCodeUnavailable(page)

      await page
        .getByRole("button", {name: /EQCNYi/})
        .first()
        .click()
      await expect(page).toHaveURL(/\/address\//)
    })

    test("switches between roots in a multi-root BoC", async ({page}) => {
      await inspect(page, CELL_INSPECTOR_FIXTURES.multiRootBase64)

      await expect(page.getByText("First root", {exact: true})).toBeVisible()

      await page.getByLabel("Root", {exact: true}).fill("1")
      await expect(page.getByText("Second root", {exact: true})).toBeVisible()
      await expect(page.getByText("First root", {exact: true})).toBeHidden()

      await expect
        .poll(() =>
          page.evaluate(() => {
            const draft = localStorage.getItem("acton:cell-inspector:draft")
            return draft ? (JSON.parse(draft) as {rootIndex?: number}).rootIndex : undefined
          }),
        )
        .toBe(1)
      const restoredPage = await page.context().newPage()
      await restoredPage.goto(route)
      await expect(restoredPage.locator("#cell-inspector-input")).toHaveValue(
        CELL_INSPECTOR_FIXTURES.multiRootBase64,
      )
      await expect(restoredPage.getByLabel("Root", {exact: true})).toHaveValue("1")
      await expect(restoredPage.getByText("Second root", {exact: true})).toBeVisible()

      await inspect(restoredPage, CELL_INSPECTOR_FIXTURES.textCommentBase64)
      await expect(restoredPage.getByLabel("Root", {exact: true})).toHaveValue("0")
      await expect(restoredPage.getByText("Hello from Cell Inspector", {exact: true})).toBeVisible()
      await restoredPage.close()
    })

    test("keeps the page usable when the root index is unavailable", async ({page}) => {
      const pageErrors: Error[] = []
      page.on("pageerror", error => pageErrors.push(error))

      await inspect(page, CELL_INSPECTOR_FIXTURES.textCommentBase64)
      await page.getByLabel("Root", {exact: true}).fill("10")

      await expect(page.getByRole("heading", {name: "Cell Inspector", exact: true})).toBeVisible()
      await expect(page.getByRole("alert")).toContainText(
        "Root 10 is unavailable. This BoC contains 1 root cell",
      )
      expect(pageErrors).toEqual([])

      await page.getByLabel("Root", {exact: true}).fill("0")
      await expect(page.getByText("Hello from Cell Inspector", {exact: true})).toBeVisible()
    })

    test("recognizes encrypted comments without attempting decryption", async ({page}) => {
      await inspect(page, CELL_INSPECTOR_FIXTURES.encryptedCommentBase64)

      await expect(page.getByText("Encrypted comment", {exact: true})).toBeVisible()
      await expect(page.getByText(/requires the recipient's private key to decrypt/i)).toBeVisible()
    })

    test("promotes real TVM code and displays verified sources", async ({page}) => {
      await installVerifiedSourceRoute(page, app)
      await inspect(page, CELL_INSPECTOR_FIXTURES.tvmCodeHex)

      const outputTabs = page.getByRole("group", {
        name: "Cell Inspector output",
      })
      await expect(outputTabs).toHaveAttribute("data-tvm-code-status", "available", {
        timeout: 15_000,
      })
      await expect(outputTabs.getByRole("button").first()).toHaveText("TVM code")
      await expect(outputTabs.getByRole("button", {name: "TVM code", exact: true})).toHaveAttribute(
        "aria-current",
        "true",
      )
      await expect(page.getByRole("button", {name: "Verified code", exact: true})).toBeVisible()
      await expect(page.getByText("main.tolk", {exact: true}).first()).toBeVisible()
      await expect(page.getByText("Verified contract code", {exact: true})).toBeVisible()
      await expect(page.getByText(/^Verified source/)).toBeVisible()
      await expect(page.getByText("exact · 100%", {exact: true})).toBeVisible()
      await expect(page.getByText("TON block.tlb · StateInit", {exact: true})).toHaveCount(0)
      await expect(page.getByText(/decoded only part of the root cell/i)).toHaveCount(0)
    })

    test("restores and persists a small cell through the URL", async ({page}) => {
      await page.goto(
        `${route}?cell=${encodeURIComponent(CELL_INSPECTOR_FIXTURES.textCommentBase64)}`,
      )

      await expect(page.locator("#cell-inspector-input")).toHaveValue(
        CELL_INSPECTOR_FIXTURES.textCommentBase64,
      )
      await expect(page.getByText("Hello from Cell Inspector", {exact: true})).toBeVisible()

      await inspect(page, CELL_INSPECTOR_FIXTURES.uint32Base64)
      await expect
        .poll(() => new URL(page.url()).searchParams.get("cell"))
        .toBe(CELL_INSPECTOR_FIXTURES.uint32Base64)

      await page.reload()
      await expect(page.locator("#cell-inspector-input")).toHaveValue(
        CELL_INSPECTOR_FIXTURES.uint32Base64,
      )
      await expect(page.getByText("Root hash", {exact: true})).toBeVisible()
    })

    test("parses a user-provided TL-B schema", async ({page}) => {
      await page.getByRole("button", {name: "Custom TL-B", exact: true}).click()
      await page.getByLabel("Custom TL-B schema", {exact: true}).fill("_ x:# = Foo;")
      await inspect(page, CELL_INSPECTOR_FIXTURES.uint32Base64)

      await expect(page.getByText("Custom TL-B schema", {exact: true})).toBeVisible()
      await expect(page.getByText(/"kind": "Foo"/)).toBeVisible()
    })

    test("automatically uses an ABI returned by the verifier", async ({page}) => {
      await installRegistryAbiRoute(page, app)
      await inspect(page, CELL_INSPECTOR_FIXTURES.abiMessageBase64)

      await expect(page.getByText("Counter · Increment", {exact: true})).toBeVisible()
      await expect(page.getByText(/Contract ABI/)).toBeVisible()
    })
  })
}

async function inspect(page: Page, value: string): Promise<void> {
  await page.locator("#cell-inspector-input").fill(value)
}

async function expectOutputTabs(page: Page): Promise<void> {
  for (const name of ["Parsed", "Raw cells", "BoC"] as const) {
    await expect(page.getByRole("button", {name, exact: true})).toBeVisible()
  }
}

async function expectTvmCodeUnavailable(page: Page): Promise<void> {
  await expect(page.getByRole("group", {name: "Cell Inspector output"})).toHaveAttribute(
    "data-tvm-code-status",
    "unavailable",
  )
  await expect(page.getByRole("button", {name: "TVM code", exact: true})).toHaveCount(0)
}

async function expectTvmCodeSecondary(page: Page): Promise<void> {
  const outputTabs = page.getByRole("group", {name: "Cell Inspector output"})
  await expect(outputTabs).toHaveAttribute("data-tvm-code-status", "available")
  await expect(outputTabs.getByRole("button").first()).toHaveText("Parsed")
  await expect(outputTabs.getByRole("button", {name: "Parsed", exact: true})).toHaveAttribute(
    "aria-current",
    "true",
  )
  await expect(outputTabs.getByRole("button", {name: "TVM code", exact: true})).toBeVisible()
}

async function installRegistryAbiRoute(
  page: Page,
  app: CellInspectorSuiteOptions["app"],
): Promise<void> {
  if (app === "localnet") {
    await page.route("**/acton_getCompilerAbi?**", route => route.fulfill({json: {}}))
  }

  await page.route("https://verifier.acton.monster/api/v1/abi?**", route =>
    route.fulfill({
      json: {
        items: [
          {
            code_hash: new URL(route.request().url()).searchParams.get("code_hash"),
            abi: REGISTRY_COMPILER_ABI,
          },
        ],
      },
    }),
  )
}

async function installVerifiedSourceRoute(
  page: Page,
  app: CellInspectorSuiteOptions["app"],
): Promise<void> {
  const source = {
    code_hash: TVM_CODE_HASH,
    verified: true,
    bundle: {
      source_bundle_hash: "cell-inspector-source",
      verified_at: 1,
      storage_revision: "test",
      entrypoint: "main.tolk",
      compiler: {language: "tolk", version: "1.2.0", params: {}},
      files: [
        {
          path: "main.tolk",
          content_hash: "main-source",
          include_in_command: true,
          is_stdlib: false,
          has_include_directives: false,
          content: "fun onInternalMessage() {}",
        },
      ],
    },
  }

  if (app === "localnet") {
    await page.route("**/acton_getRegisteredVerifiedSource?**", route =>
      route.fulfill({json: source}),
    )
    return
  }

  await page.route("https://verifier.acton.monster/api/v1/verification/source?**", route =>
    route.fulfill({json: source}),
  )
}
