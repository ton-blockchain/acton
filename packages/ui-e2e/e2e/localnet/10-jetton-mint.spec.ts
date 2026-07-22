import {expect, test} from "@playwright/test"

import {JETTON_MASTER_ADDRESS, mockJettonMaster} from "../support/jettonMaster"
import {prepareVisualPage} from "../support/visual"

test.describe("Jetton master mint action", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "localnet"})
  })

  test("opens the faucet with a mintable master selected", async ({page}) => {
    await mockJettonMaster(page, true)
    await page.goto(`/explorer/address/${JETTON_MASTER_ADDRESS}`)

    await page.getByRole("button", {name: "Mint token"}).click()

    await expect(page).toHaveURL(`/faucet?jetton=${encodeURIComponent(JETTON_MASTER_ADDRESS)}`)
    await expect(page.getByRole("button", {name: "Choose faucet asset"})).toContainText("VIS")
    await expect(page.getByRole("button", {name: "Mint Jetton"})).toBeVisible()
  })

  test("hides the action for a non-mintable master", async ({page}) => {
    await mockJettonMaster(page, false)
    await page.goto(`/explorer/address/${JETTON_MASTER_ADDRESS}`)

    await expect(page.getByRole("button", {name: "Metadata"})).toBeVisible()
    await expect(page.getByRole("button", {name: "Mint token"})).toHaveCount(0)
  })
})
