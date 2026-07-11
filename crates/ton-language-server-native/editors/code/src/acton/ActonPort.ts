//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as net from "node:net"

const LOOPBACK_HOST = "127.0.0.1"

export async function getFreeActonPort(host: string = LOOPBACK_HOST): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer()

    server.once("error", reject)
    server.listen(0, host, () => {
      const address = server.address()
      if (!address || typeof address === "string") {
        server.close()
        reject(new Error("Failed to allocate a local port."))
        return
      }

      server.close(closeError => {
        if (closeError) {
          reject(closeError)
          return
        }

        resolve(address.port)
      })
    })
  })
}
