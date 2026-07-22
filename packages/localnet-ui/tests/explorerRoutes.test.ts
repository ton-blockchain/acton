import {describe, expect, test} from "bun:test"

import {createExplorerRoutes} from "../src/explorer/hooks/explorerRoutesContext"

const RAW_ADDRESS = "-1:5555555555555555555555555555555555555555555555555555555555555555"

describe("explorer account routes", () => {
  test("encodes account links in the user-friendly format for the selected network", () => {
    const mainnetRoutes = createExplorerRoutes("", {testOnly: false})
    const testnetRoutes = createExplorerRoutes("/explorer", {testOnly: true})
    const unbounceableRoutes = createExplorerRoutes("", {
      bounceable: false,
      testOnly: false,
    })

    expect({
      mainnet: mainnetRoutes.addressPath(RAW_ADDRESS),
      testnet: testnetRoutes.addressPath(RAW_ADDRESS),
      unbounceable: unbounceableRoutes.addressPath(RAW_ADDRESS),
      networkChanged: mainnetRoutes.addressPath("kf9VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVQft"),
      invalidPreserved: mainnetRoutes.addressPath("not an address"),
    }).toMatchInlineSnapshot(`
      {
        "invalidPreserved": "/address/not%20an%20address",
        "mainnet": "/address/Ef9VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVbxn",
        "networkChanged": "/address/Ef9VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVbxn",
        "testnet": "/explorer/address/kf9VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVQft",
        "unbounceable": "/address/Uf9VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVeGi",
      }
    `)
  })
})
