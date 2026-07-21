import {expect, test} from "@playwright/test"

import {visualSnapshotsEnabled} from "../support/visual"

const previews = [
  {key: "home", route: "/"},
  {key: "blocks", route: "/blocks"},
  {key: "block", route: "/block/-1/8000000000000000/123"},
  {key: "abi", route: "/abi"},
  {key: "sources", route: "/sources"},
  {key: "cell", route: "/cell"},
  {key: "emulate", route: "/emulate"},
  {key: "favorites", route: "/favorites"},
  {key: "transaction", route: `/tx/${"0".repeat(64)}`},
] as const

test.describe("Explorer Open Graph previews", () => {
  test("serves route-specific metadata", async ({request}) => {
    for (const preview of previews) {
      const response = await request.get(preview.route)
      expect(response.ok(), preview.route).toBeTruthy()

      const html = await response.text()
      expect(html).toContain(`/og/page.png?page=${preview.key}&amp;v=6`)
    }
  })

  test.describe("visual snapshots", () => {
    test.skip(!visualSnapshotsEnabled, "Set CHECK_UI_SNAPSHOTS=1 on macOS")

    for (const preview of previews) {
      test(`exp-og-${preview.key}`, async ({page}) => {
        const response = await page.goto(`/og/page.png?page=${preview.key}`)
        expect(response?.headers()["content-type"]).toContain("image/png")

        const image = page.locator("img")
        await expect(image).toHaveJSProperty("naturalWidth", 1200)
        await expect(image).toHaveJSProperty("naturalHeight", 630)
        await expect(image).toHaveScreenshot(`exp-og-${preview.key}.png`, {
          animations: "disabled",
          caret: "hide",
          maxDiffPixels: 200,
        })
      })
    }
  })
})
