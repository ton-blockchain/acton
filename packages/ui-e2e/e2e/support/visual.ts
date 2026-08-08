import process from "node:process"

import {expect, type Page} from "@playwright/test"

export type VisualApp = "explorer" | "localnet"
export type VisualTheme = "dark" | "light"

interface PrepareVisualPageOptions {
  readonly app: VisualApp
  readonly storage?: Readonly<Record<string, string>>
  readonly theme?: VisualTheme
}

interface ScreenshotOptions {
  readonly fullPage?: boolean
}

export const visualSnapshotsEnabled =
  process.platform === "darwin" && process.env.CHECK_UI_SNAPSHOTS === "1"

const localnetNodePort = Number(process.env.ACTON_UI_E2E_NODE_PORT ?? 15_411)

export const localnetBaseUrl = `http://127.0.0.1:${localnetNodePort}`

export const explorerLocalnetStorage = (): Readonly<Record<string, string>> => {
  const v2BaseUrl = `${localnetBaseUrl}/api/v2`
  const v3BaseUrl = `${localnetBaseUrl}/api/v3`
  const id = `custom:${encodeURIComponent(v3BaseUrl)}`
  return {
    explorerNetwork: id,
    explorerCustomNetworks: JSON.stringify([
      {
        id,
        label: "Visual localnet",
        testOnly: true,
        supportsActions: true,
        api: {v2BaseUrl, v3BaseUrl},
      },
    ]),
  }
}

export const prepareVisualPage = async (
  page: Page,
  {app, storage = {}, theme = "light"}: PrepareVisualPageOptions,
) => {
  await page.addInitScript(
    ({appName, initialStorage, initialTheme}) => {
      localStorage.clear()
      localStorage.setItem(appName === "explorer" ? "explorerTheme" : "theme", initialTheme)
      for (const [key, value] of Object.entries(initialStorage)) {
        localStorage.setItem(key, value)
      }
    },
    {appName: app, initialStorage: storage, initialTheme: theme},
  )
}

export const delayRealLocalnetApiResponses = async (page: Page, responseDelayMs: number) => {
  await page.route(
    url => url.pathname.startsWith("/api/"),
    async route => {
      await new Promise(resolve => setTimeout(resolve, responseDelayMs))
      await route.continue()
    },
  )
}

const stabilizeVisualPage = async (page: Page) => {
  await page.evaluate(async () => {
    await document.fonts.ready
    window.scrollTo({left: 0, top: 0})

    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur()
    }

    for (const element of document.querySelectorAll<HTMLElement>("[data-visual-dynamic]")) {
      const placeholder = element.dataset.visualPlaceholder ?? "<dynamic>"
      element.replaceChildren(document.createTextNode(placeholder))
      element.setAttribute("title", placeholder)
    }

    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
  })
}

export const expectVisualSnapshot = async (
  page: Page,
  scenarioId: string,
  {fullPage = true}: ScreenshotOptions = {},
) => {
  await stabilizeVisualPage(page)
  await expect(page).toHaveScreenshot(`${scenarioId}.png`, {
    animations: "disabled",
    caret: "hide",
    fullPage,
    maxDiffPixels: 200,
  })
}
