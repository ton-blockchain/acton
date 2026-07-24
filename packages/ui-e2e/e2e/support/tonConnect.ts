import type {Page} from "@playwright/test"

const testWalletMnemonic =
  "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later".split(
    " ",
  )

export const mockTonConnectStartupWallet = async (page: Page, address: string) => {
  await page.route(
    url => url.pathname === "/acton_getStartupWallets",
    async route =>
      route.fulfill({
        json: [
          {
            name: "deployer",
            mnemonic: testWalletMnemonic,
            version: "v4r2",
            network: "localnet",
            address,
            public_key: "",
            wallet_id: 698_983_191,
          },
        ],
      }),
  )
}
