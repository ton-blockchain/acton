import {expect, test} from "@playwright/test"

import {
  delayRealLocalnetApiResponses,
  expectVisualSnapshot,
  prepareVisualPage,
  visualSnapshotsEnabled,
} from "../support/visual"

test.describe("Localnet blocks from the canonical snapshot", () => {
  test.beforeEach(async ({page}) => {
    await prepareVisualPage(page, {app: "localnet"})
  })

  test("renders blocks and transactions from the real node", async ({page}) => {
    await page.goto("/explorer/blocks")
    await expect(page.getByRole("heading", {name: "Blocks", exact: true})).toBeVisible()
    await expect(page.getByRole("region", {name: "Last masterchain blocks"})).toBeVisible()
    await expect(page.getByRole("region", {name: "Last workchain blocks"})).toBeVisible()
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    test("loc-blocks-loading", async ({page}) => {
      await delayRealLocalnetApiResponses(page, 2000)
      await page.goto("/explorer/blocks")
      await expect(
        page.getByRole("region", {name: "Loading Last masterchain blocks"}),
      ).toBeVisible()
      await expectVisualSnapshot(page, "loc-blocks-loading")
    })

    test("loc-blocks-populated", async ({page}) => {
      await page.goto("/explorer/blocks")
      await expect(page.getByRole("region", {name: "Last masterchain blocks"})).toBeVisible()
      await expectVisualSnapshot(page, "loc-blocks-populated")
    })
  })
})
