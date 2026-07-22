import process from "node:process"
import {Buffer} from "node:buffer"

import {expect, test} from "@playwright/test"

import {
  SIGN_DATA_CELL,
  SIGN_DATA_CELL_SCHEMA,
  SIGN_DATA_TEXT,
} from "../../fixtures/tonconnect-dapp/signDataPayloads"
import {mockTonConnectStartupWallet} from "../support/tonConnect"
import {prepareVisualPage, visualSnapshotsEnabled} from "../support/visual"

const dappPort = Number(process.env.ACTON_UI_E2E_TONCONNECT_DAPP_PORT ?? 14_308)
const dappBaseUrl = `http://127.0.0.1:${dappPort}`
const testWalletAddress = "0:513ec97b0c602901c3cf14ac0aa588292468969cccd0d84a4c3fb81e7f897a9c"
const testWalletFriendlyAddress = "EQBRPsl7DGApAcPPFKwKpYgpJGiWnMzQ2EpMP7gef4l6nHTG"
test("connects a temporary dApp and signs data", async ({context, page}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"])
  await prepareVisualPage(page, {app: "localnet"})
  await mockTonConnectStartupWallet(page, testWalletAddress)

  const dappPage = await context.newPage()
  await dappPage.goto(dappBaseUrl)
  await dappPage.getByRole("button", {name: "Create connection"}).click()
  const tonConnectUrl = await dappPage.getByLabel("TON Connect URL").inputValue()
  expect(tonConnectUrl).toMatch(/^tc:\/\//)

  await page.goto("/wallets")
  await expect(page.getByLabel("Connect URL")).toBeEnabled()
  await page.getByLabel("Connect URL").fill(tonConnectUrl)
  await page.getByRole("button", {name: "Handle request"}).click()

  const connectDialog = page.getByRole("dialog", {name: "Connection Request"})
  await expect(connectDialog).toContainText("Acton signData E2E dApp wants to connect")
  await connectDialog.getByRole("button", {name: "Connect", exact: true}).click()
  await expect(dappPage.getByRole("status")).toContainText(`Connected: ${testWalletAddress}`)

  await dappPage.getByRole("button", {name: "Request text signature"}).click()

  const signDialog = page.getByRole("dialog", {name: "Sign Request", exact: true})
  await expect(signDialog).toContainText("Acton signData E2E dApp wants a signature")
  await expect(signDialog).toContainText("Text")
  await expect(signDialog).toContainText(`· ${SIGN_DATA_TEXT.length} characters`)
  await expect(signDialog).toContainText("Message")
  await expect(signDialog).toContainText(SIGN_DATA_TEXT)
  await expect(signDialog.getByRole("button", {name: "Reject"})).toBeVisible()
  await expect(signDialog.getByRole("button", {name: "Sign", exact: true})).toBeVisible()

  if (visualSnapshotsEnabled) {
    await page.evaluate(async () => document.fonts.ready)
    await expect(signDialog).toHaveScreenshot("loc-ton-connect-sign-data-form.png", {
      animations: "disabled",
      caret: "hide",
      maxDiffPixels: 100,
    })
  }

  await signDialog.getByRole("button", {name: "Copy text payload"}).click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(SIGN_DATA_TEXT)

  await signDialog.getByRole("button", {name: "Sign", exact: true}).click()
  await expect(dappPage.getByRole("status")).toHaveText("Text signature received")

  const textSignatureResult = JSON.parse(
    await dappPage.getByLabel("Signature result").textContent(),
  ) as SignDataResult
  expect(textSignatureResult).toMatchObject({
    address: testWalletAddress,
    payload: {
      type: "text",
      text: SIGN_DATA_TEXT,
      network: "-3",
      from: testWalletAddress,
    },
  })
  expectValidSignatureResult(textSignatureResult)

  await dappPage.getByRole("button", {name: "Request cell signature"}).click()

  const cellSignDialog = page.getByRole("dialog", {name: "Sign Request", exact: true})
  await expect(cellSignDialog).toContainText("ActonSignRequest")
  await expect(cellSignDialog).toContainText("· 174 bits · 2 refs")
  await expect(cellSignDialog).toContainText("Parsed Cell")
  await expect(cellSignDialog).toContainText("1.25 GRAM (1250000000 nano)")
  await expect(cellSignDialog).toContainText("42 (0x2a)")
  await expect(cellSignDialog).toContainText("165 (0b10100101)")
  await expect(cellSignDialog).toContainText("true")
  await expect(cellSignDialog).toContainText("Maybe Just")
  await expect(cellSignDialog).toContainText("1900000000 (0x713fb300)")
  await expect(cellSignDialog).toContainText("ActonSignDetails")
  await expect(cellSignDialog).toContainText("EQBRPs…l6nHTG")
  await expect(cellSignDialog).toContainText("-1337")
  await expect(cellSignDialog).toContainText(
    "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  )
  await expect(cellSignDialog).toContainText("ActonSignAudit")
  await expect(cellSignDialog).toContainText("1700000000 (0x6553f100)")
  await expect(cellSignDialog).toContainText("9007199254740993 (0x20000000000001)")
  await expect(cellSignDialog).toContainText("false")
  await expect(cellSignDialog).toContainText("16 bits, 1 refs")
  await cellSignDialog.getByText("Schema", {exact: true}).click()
  await expect(cellSignDialog).toContainText(SIGN_DATA_CELL_SCHEMA)

  if (visualSnapshotsEnabled) {
    await page.evaluate(async () => document.fonts.ready)
    await expect(cellSignDialog).toHaveScreenshot("loc-ton-connect-sign-data-cell-form.png", {
      animations: "disabled",
      caret: "hide",
      maxDiffPixels: 100,
    })
  }

  await cellSignDialog.getByRole("button", {name: "Copy cell BoC as hex"}).click()
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe(Buffer.from(SIGN_DATA_CELL, "base64").toString("hex"))

  await cellSignDialog.getByText("EQBRPs…l6nHTG").hover()
  await cellSignDialog.getByRole("button", {name: "Copy address"}).click()
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe(testWalletFriendlyAddress)

  await cellSignDialog.getByRole("button", {name: "Sign", exact: true}).click()
  await expect(dappPage.getByRole("status")).toHaveText("Cell signature received")

  const cellSignatureResult = JSON.parse(
    await dappPage.getByLabel("Signature result").textContent(),
  ) as SignDataResult
  expect(cellSignatureResult).toMatchObject({
    address: testWalletAddress,
    payload: {
      type: "cell",
      schema: SIGN_DATA_CELL_SCHEMA,
      cell: SIGN_DATA_CELL,
      network: "-3",
      from: testWalletAddress,
    },
  })
  expectValidSignatureResult(cellSignatureResult)
})

interface SignDataResult {
  readonly signature: string
  readonly address: string
  readonly timestamp: number
  readonly domain: string
  readonly payload: Readonly<Record<string, unknown>>
}

function expectValidSignatureResult(result: SignDataResult): void {
  expect(result.signature).toMatch(/^[A-Za-z0-9+/]{86}==$/)
  expect(result.timestamp).toBeGreaterThan(0)
  expect(result.domain).toContain("127.0.0.1")
}
