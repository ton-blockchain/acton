import {expect, test} from "@playwright/test"

type SmokeGlobal = typeof globalThis & {
  __tlbSmoke?: {
    definitionAtReference: () => Promise<unknown>
    editorText: () => string
    languageId: () => string | undefined
  }
}

test("Monaco resolves TL-B definitions through the local WASM language server", async ({page}) => {
  await page.goto("/")
  await expect(page.locator("#status")).toHaveText("ready: 2 definitions")

  const languageId = await page.evaluate(() => (globalThis as SmokeGlobal).__tlbSmoke?.languageId())
  expect(languageId).toBe("tlb")

  const editorText = await page.evaluate(() => (globalThis as SmokeGlobal).__tlbSmoke?.editorText())
  expect(editorText).toContain("baz$2 x:CommonMsgInfo = Wrap;")

  const definitions = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tlbSmoke?.definitionAtReference(),
  )
  expect(definitions).toEqual([
    {
      uri: "file:///workspace/main.tlb",
      range: {
        start: {line: 0, character: 12},
        end: {line: 0, character: 25},
      },
    },
    {
      uri: "file:///workspace/main.tlb",
      range: {
        start: {line: 1, character: 12},
        end: {line: 1, character: 25},
      },
    },
  ])
})
