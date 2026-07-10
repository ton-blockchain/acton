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
    definitionAt: (
      line: number,
      character: number,
    ) => Promise<
      {
        uri: string
        range: {
          start: {line: number; character: number}
          end: {line: number; character: number}
        }
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
    foldingRanges: () => Promise<{start: number; end: number; kind: string | null}[]>
    inlayHints: () => Promise<
      {
        position: {line: number; character: number}
        label: string
        kind?: number
        tooltip?: string
      }[]
    >
    completionAt: (line: number, character: number) => Promise<{label: string; detail?: string}[]>
    actonTomlCompletionAt: (
      line: number,
      character: number,
    ) => Promise<{label: string; detail?: string}[]>
    actonTomlHoverAt: (
      line: number,
      character: number,
    ) => Promise<
      {
        contents: readonly string[]
        range: {
          start: {line: number; character: number}
          end: {line: number; character: number}
        } | null
      }[]
    >
    applyCompletionAt: (line: number, character: number, label: string) => Promise<boolean>
    logs: () => Promise<string>
    profile: () => Promise<string>
    sidePanelText: () => string
    logsPanelText: () => string
    profilePanelText: () => string
    editorText: () => string
    actonTomlText: () => string
    languageId: () => string | undefined
    actonTomlLanguageId: () => string | undefined
    selectedLanguage: () => "tolk" | "tasm" | "tlb" | "fift"
    setEditorText: (text: string) => void
    setActonTomlText: (text: string) => void
    setLanguage: (languageId: "tolk" | "tasm" | "tlb" | "fift") => Promise<void>
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
  await expect(page.locator("#status")).toHaveText(/Tolk saved locally, 0 hovers/)

  const languageId = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.languageId(),
  )
  expect(languageId).toBe("tolk")

  const editorText = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.editorText(),
  )
  expect(editorText).toContain("struct Storage")

  const tolkSource = `struct Storage {
    counter: int
}
fun Storage.save(self) {}
fun main() {
    var storage = Storage { counter: 1 };
    storage.save();
    storage.counter;
}
`
  await page.evaluate(source => {
    ;(globalThis as SmokeGlobal).__tonLsSmoke?.setEditorText(source)
  }, tolkSource)
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toBe(tolkSource)

  const saveDefinitions = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.definitionAt(6, 12),
  )
  expect(saveDefinitions).toContainEqual(
    expect.objectContaining({
      uri: "file:///workspace/main.tolk",
      range: expect.objectContaining({
        start: {line: 3, character: 12},
      }),
    }),
  )

  const fieldDefinitions = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.definitionAt(7, 12),
  )
  expect(fieldDefinitions).toContainEqual(
    expect.objectContaining({
      uri: "file:///workspace/main.tolk",
      range: expect.objectContaining({
        start: {line: 1, character: 4},
      }),
    }),
  )

  const completionSource = `struct Storage {
    counter: int
}
fun main() {
    var storage = Storage { counter: 1 };
    storage.
}
`
  await page.evaluate(source => {
    ;(globalThis as SmokeGlobal).__tonLsSmoke?.setEditorText(source)
  }, completionSource)
  await expect
    .poll(async () => {
      const items = await page.evaluate(() =>
        (globalThis as SmokeGlobal).__tonLsSmoke?.completionAt(5, 12),
      )
      return items?.map(item => item.label)
    })
    .toContain("counter")
  expect(
    await page.evaluate(() =>
      (globalThis as SmokeGlobal).__tonLsSmoke?.applyCompletionAt(5, 12, "counter"),
    ),
  ).toBe(true)
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toContain("storage.counter")

  const inlaySource = `const COMPUTED = 1 + 2
fun deliver(destination: int): void {}
get fun counter(): int { return 0 }
fun main(): void {
    deliver(1);
}
`
  await page.evaluate(source => {
    ;(globalThis as SmokeGlobal).__tonLsSmoke?.setEditorText(source)
  }, inlaySource)
  await expect
    .poll(async () => {
      const hints = await page.evaluate(() =>
        (globalThis as SmokeGlobal).__tonLsSmoke?.inlayHints(),
      )
      return hints?.map(hint => hint.label)
    })
    .toEqual(
      expect.arrayContaining([
        ": int",
        " /* = 3 (0x3) */",
        "destination:",
        expect.stringMatching(/^\(0x[0-9a-f]+\)$/),
      ]),
    )

  const valueHint = await page.evaluate(async () => {
    const hints = await (globalThis as SmokeGlobal).__tonLsSmoke?.inlayHints()
    return hints?.find(hint => hint.label.includes("/* ="))
  })
  expect(valueHint).toMatchObject({
    tooltip: "Evaluated value: 3 (0x3)",
  })
  expect(valueHint).not.toHaveProperty("kind")

  await page.selectOption("#language-select", "tasm")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.selectedLanguage()))
    .toBe("tasm")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toContain("DICTPUSHCONST 19")

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

  await page.selectOption("#language-select", "fift")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.selectedLanguage()))
    .toBe("fift")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toContain("PROGRAM{")
  const foldingRanges = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.foldingRanges(),
  )
  expect(foldingRanges).toContainEqual({start: 0, end: 16, kind: null})
  expect(foldingRanges).toContainEqual({start: 2, end: 15, kind: null})
  expect(foldingRanges).toContainEqual({start: 3, end: 4, kind: null})
  expect(foldingRanges).toContainEqual({start: 5, end: 6, kind: null})
  expect(foldingRanges).toContainEqual({start: 8, end: 9, kind: null})
  expect(foldingRanges).toContainEqual({start: 10, end: 11, kind: null})
  expect(foldingRanges?.length).toBeGreaterThanOrEqual(5)

  const actonToml = `[lint]
output-format = "json"
`
  await page.evaluate(source => {
    ;(globalThis as SmokeGlobal).__tonLsSmoke?.setActonTomlText(source)
  }, actonToml)
  expect(
    await page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.actonTomlLanguageId()),
  ).toBe("toml")
  await expect
    .poll(async () => {
      const items = await page.evaluate(() =>
        (globalThis as SmokeGlobal).__tonLsSmoke?.actonTomlCompletionAt(1, 18),
      )
      return items?.map(item => item.label)
    })
    .toEqual(expect.arrayContaining(['"plain"', '"json"', '"sarif"', '"github"', '"gitlab"']))

  const tomlHovers = await page.evaluate(() =>
    (globalThis as SmokeGlobal).__tonLsSmoke?.actonTomlHoverAt(1, 2),
  )
  expect(tomlHovers?.[0]?.contents.join("\n")).toContain("Output format for `acton check`")
  expect(tomlHovers?.[0]?.contents.join("\n")).toContain('`"plain" | "json"')

  await page.selectOption("#language-select", "tasm")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.selectedLanguage()))
    .toBe("tasm")

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
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.actonTomlText()))
    .toBe(actonToml)

  await page.selectOption("#language-select", "tasm")
  await expect
    .poll(() => page.evaluate(() => (globalThis as SmokeGlobal).__tonLsSmoke?.editorText()))
    .toBe("SUB\nADD")
})
