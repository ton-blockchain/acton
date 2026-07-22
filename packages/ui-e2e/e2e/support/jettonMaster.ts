import type {Page} from "@playwright/test"

export const JETTON_MASTER_ADDRESS =
  "0:3029b3eaeda86a5381d86100f2a8b761c38de45642edb6e4bb1cca2e6dd7ffed"

export const mockJettonMaster = async (page: Page, mintable: boolean) => {
  await page.route(
    url => url.pathname === "/api/v3/accountStates",
    async route => {
      await route.fulfill({
        json: {
          accounts: [
            {
              account_state_hash:
                "27d4165a1cdeef6b298faeaa63c0cf34df07a8863ae89ea27167364812043edc",
              address: JETTON_MASTER_ADDRESS,
              balance: "99998916463",
              code_hash: "20834b7b72b112147e1b2fb457b84e74d1a30f04f737d4f62a668e9552d2b72f",
              contract_methods: [],
              data_hash: "5f982be619c78e7a1e49306cdfd880d6eaafc7a07efc573436083a68ef3621e4",
              extra_currencies: {},
              interfaces: ["jetton_master"],
              last_transaction_hash:
                "a17dcd62e4136c053b59bd76092803a30553895b788f288b1a5f02e50d30922d",
              last_transaction_lt: "3",
              status: "active",
            },
          ],
          address_book: {},
          metadata: {},
        },
      })
    },
  )

  await page.route(
    url => url.pathname === "/api/v3/jetton/masters",
    async route => {
      await route.fulfill({
        json: {
          jetton_masters: [
            {
              address: JETTON_MASTER_ADDRESS,
              admin_address: JETTON_MASTER_ADDRESS,
              code_hash: "20834b7b72b112147e1b2fb457b84e74d1a30f04f737d4f62a668e9552d2b72f",
              data_hash: "5f982be619c78e7a1e49306cdfd880d6eaafc7a07efc573436083a68ef3621e4",
              jetton_content: {
                decimals: "9",
                name: "Visual Token",
                symbol: "VIS",
              },
              jetton_wallet_code_hash:
                "1111111111111111111111111111111111111111111111111111111111111111",
              last_transaction_lt: "3",
              mintable,
              total_supply: "1000000000",
            },
          ],
          metadata: {},
        },
      })
    },
  )
}
