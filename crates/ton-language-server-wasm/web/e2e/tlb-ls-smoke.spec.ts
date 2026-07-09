import {expect, test} from "@playwright/test"

type SmokeGlobal = typeof globalThis & {
  __tonLsSmoke?: {
    hoverAtInstruction: () => Promise<
      {
        contents: readonly string[]
        range: {
          start: {line: number; character: number}
          end: {line: number; character: number}
        } | null
      }[]
    >
    codeLenses: () => Promise<
      {
        title: string | null
        command: string | null
        range: {
          start: {line: number; character: number}
          end: {line: number; character: number}
        }
      }[]
    >
    logs: () => Promise<string>
    profile: () => Promise<string>
    sidePanelText: () => string
    logsPanelText: () => string
    profilePanelText: () => string
    editorText: () => string
    languageId: () => string | undefined
    selectedLanguage: () => "tasm" | "tlb"
    setEditorText: (text: string) => void
    setLanguage: (languageId: "tasm" | "tlb") => Promise<void>
    setProfileVisible: (visible: boolean) => void
  }
}

const codeLensProfileCount = (text: string | undefined) =>
  Number(/code_lens: count=(\d+)/.exec(text ?? "")?.[1] ?? 0)

test("Monaco runs the local WASM language server with persisted files and logs", async ({page}) => {
  await page.addInitScript(() => {
    const marker = "__tonLsE2eStorageCleared"
    if (!sessionStorage.getItem(marker)) {
      localStorage.clear()
      sessionStorage.setItem(marker, "1")
    }
  })
  await page.goto("/")
  await expect(page.locator("#status")).toHaveText(/TASM saved locally, 1 hovers/)

  const languageId = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.languageId(),
  )
  expect(languageId).toBe("tasm")

  const editorText = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.editorText(),
  )
  expect(editorText).toContain("DICTPUSHCONST 19")

  const hovers = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.hoverAtInstruction(),
  )
  expect(hovers?.[0]?.contents.join("\n")).toContain("```")
  expect(hovers?.[0]?.contents.join("\n")).toContain("- Stack")
  expect(hovers?.[0]?.contents.join("\n")).toContain("SETCP")

  const codeLenses = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.codeLenses(),
  )
  expect(codeLenses?.length).toBeGreaterThan(10)
  expect(codeLenses?.[0]).toMatchObject({
    command: "tonls.tasm.stackEffect",
    range: {
      start: {line: 0, character: 0},
    },
  })

  await page.selectOption("#log-level-select", "debug")
  await page.check("#logs-toggle")
  await expect(page.locator("#logs-editor-root")).toBeVisible()
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.logs()))
    .toContain("logging.set_level")
  await page.getByRole("button", {name: "Clear"}).click()
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.logs()))
    .toBe("")

  await page.check("#profile-toggle")
  await expect(page.locator("#profile-editor-root")).toBeVisible()
  await expect(page.locator("#logs-editor-root")).toBeVisible()
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.profilePanelText()))
    .toContain("Spans")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.profile()))
    .toContain("code_lens")

  const beforeCodeLensCount = codeLensProfileCount(
    await page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.profilePanelText()),
  )
  await page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.codeLenses())
  await expect
    .poll(async () =>
      codeLensProfileCount(
        await page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.profilePanelText()),
      ),
    )
    .toBeGreaterThan(beforeCodeLensCount)

  await page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.setEditorText("SUB\nADD"))
  await page.selectOption("#language-select", "tlb")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.selectedLanguage()))
    .toBe("tlb")
  await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.setEditorText("one$0 a:# = SavedTlb;\n"),
  )

  await page.reload()
  await expect(page.locator("#status")).toHaveText(/TL-B saved locally/)
  await expect(page.locator("#language-select")).toHaveValue("tlb")
  await expect(page.locator("#log-level-select")).toHaveValue("debug")
  await expect(page.locator("#logs-toggle")).toBeChecked()
  await expect(page.locator("#profile-toggle")).toBeChecked()
  await expect(page.locator("#logs-editor-root")).toBeVisible()
  await expect(page.locator("#profile-editor-root")).toBeVisible()
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toBe("one$0 a:# = SavedTlb;\n")

  await page.selectOption("#language-select", "tasm")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toBe("SUB\nADD")
})
