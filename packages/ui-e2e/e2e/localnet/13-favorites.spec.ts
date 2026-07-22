import {expect, test} from "@playwright/test"

const transactionHash = "a17dcd62e4136c053b59bd76092803a30553895b788f288b1a5f02e50d30922d"

test("adds a transaction to favorites and keeps it across reloads", async ({page}) => {
  await page.goto(`/explorer/tx/${transactionHash}`)

  const favoriteButton = page.getByRole("button", {name: "Add to favorites"})
  await expect(favoriteButton).toBeVisible()
  await favoriteButton.click()
  await expect(page.getByRole("button", {name: "Remove from favorites"})).toBeVisible()

  await page.getByRole("button", {name: "Favorites", exact: true}).click()
  await expect(page).toHaveURL(/\/explorer\/favorites$/)

  const transactions = page.getByRole("region", {name: "Favorite transactions"})
  await expect(transactions.getByRole("link", {name: "a17dcd…30922d"})).toBeVisible()
  await expect(transactions.getByText("3", {exact: true})).toBeVisible()

  await page.reload()
  await expect(page.getByRole("region", {name: "Favorite transactions"})).toBeVisible()

  await page
    .getByRole("region", {name: "Favorite transactions"})
    .getByRole("button", {name: "Remove from favorites"})
    .click()
  await expect(page.getByText("No favorites yet", {exact: true})).toBeVisible()
})
