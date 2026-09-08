// biome-ignore lint/correctness/noUndeclaredDependencies: Playwright is shared from the workspace root.
import {expect, test} from "@playwright/test"

import {verificationScenario} from "./verification.scenario"

test("verification reviews sources and separates publication from payment in each network", async ({
  page,
}) => {
  const result = await verificationScenario(page)

  expect(result).toMatchSnapshot("verification-flow.txt")
})
