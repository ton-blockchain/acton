// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"

for (const kind of ["actonSimulatedLocalnet", "fullTonNetwork"]) {
  test(`${kind} selects and submits the same startup wallets`, async ({page}) => {
    await page.setViewportSize({width: 1100, height: 560})
    await page.route("**/api/v1/**", async route => {
      const path = new URL(route.request().url()).pathname
      if (path === "/api/v1/info") {
        await route.fulfill({
          json: {
            protocolVersion: 1,
            serverVersion: "test",
            workspace: {name: "test", walletNames: ["deployer", "treasury"]},
          },
        })
      } else if (path === "/api/v1/environments" && route.request().method() === "POST") {
        const request = route.request().postDataJSON()
        await route.fulfill({
          status: 201,
          json: {
            id: "created",
            name: request.name,
            config: {
              port: 5411,
              apiV2Port: 18_080,
              apiV3Port: 18_081,
              adminPort: 18_082,
              configPort: 18_083,
              observabilityPort: 18_084,
              nodes: [],
              ...request.config,
            },
            status: "stopped",
            lifecycle: "managed",
            rpcUrl: "/rpc",
            capabilities: [],
            endpoints: {},
            network: {id: "local", label: "Local", chainId: -3, testOnly: true},
          },
        })
      } else {
        await route.fulfill({json: []})
      }
    })

    await page.goto("/virtual-environments")
    await page.getByRole("button", {name: "Create environment", exact: true}).first().click()
    const dialog = page.getByRole("dialog", {name: "Create environment", exact: true})
    await dialog.getByLabel("Environment type").selectOption(kind)
    await expect(
      dialog.getByText("Initialize selected project wallets and fund each with 100 GRAM"),
    ).toBeVisible()
    await dialog.getByLabel("Startup accounts").click()
    const suggestion = page.getByRole("option", {name: "treasury", exact: true})
    await expect(suggestion).toBeVisible()
    // The dropdown must escape the dialog's scrolling body and footer.
    expect(
      await suggestion.evaluate(element => {
        const box = element.getBoundingClientRect()
        const hit = document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)
        return !element.closest('[role="dialog"]') && hit !== null && element.contains(hit)
      }),
    ).toBe(true)
    await page.getByRole("option", {name: "deployer", exact: true}).click()

    // Changing the network type must keep the user's selection.
    await dialog
      .getByLabel("Environment type")
      .selectOption(kind === "fullTonNetwork" ? "actonSimulatedLocalnet" : "fullTonNetwork")
    await dialog.getByLabel("Environment type").selectOption(kind)
    await expect(dialog.getByRole("button", {name: "Remove deployer"})).toBeVisible()
    await dialog.getByLabel("Startup accounts").click()
    await dialog.getByLabel("Startup accounts").fill("treas")
    await expect(page.getByRole("option", {name: "treasury", exact: true})).toBeVisible()
    await dialog.getByLabel("Startup accounts").press("Enter")
    await expect(dialog).toBeVisible()
    await dialog.getByRole("button", {name: "Remove deployer"}).click()
    await dialog.getByLabel("Startup accounts").press("Escape")
    await expect(page.getByRole("listbox")).not.toBeVisible()

    const submitted = page.waitForRequest(
      request => request.url().endsWith("/api/v1/environments") && request.method() === "POST",
    )
    await dialog.getByRole("button", {name: "Create environment", exact: true}).click()
    expect((await submitted).postDataJSON().config).toMatchObject({kind, accounts: ["treasury"]})
    await expect(dialog).not.toBeVisible()
  })
}
