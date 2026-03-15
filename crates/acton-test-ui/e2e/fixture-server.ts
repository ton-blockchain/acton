import {createServer, type ServerResponse} from "node:http"

import {config, contracts, fileContents, reports, traces} from "./fixture-data"

const host = "127.0.0.1"
const port = Number.parseInt(process.env.ACTON_TEST_UI_API_PORT ?? "4310", 10)

function sendJson(res: ServerResponse, body: unknown, status = 200): void {
  res.writeHead(status, {"content-type": "application/json"})
  res.end(JSON.stringify(body))
}

function sendText(res: ServerResponse, body: string, status = 200): void {
  res.writeHead(status, {"content-type": "text/plain; charset=utf-8"})
  res.end(body)
}

const server = createServer((req, res) => {
  const url = new URL(req.url ?? "/", `http://${req.headers.host ?? `${host}:${port}`}`)

  if (req.method !== "GET") {
    sendJson(res, {error: "Method not allowed"}, 405)
    return
  }

  if (url.pathname === "/api/config") {
    sendJson(res, config)
    return
  }

  if (url.pathname === "/api/reports") {
    sendJson(res, reports)
    return
  }

  if (url.pathname.startsWith("/api/trace/")) {
    const traceName = decodeURIComponent(url.pathname.slice("/api/trace/".length))
    const trace = traces[traceName]
    if (trace === undefined) {
      sendJson(res, {error: "Trace not found"}, 404)
      return
    }
    sendJson(res, trace)
    return
  }

  if (url.pathname.startsWith("/api/contract/")) {
    const contractName = decodeURIComponent(url.pathname.slice("/api/contract/".length))
    const contract = contracts[contractName]
    if (contract === undefined) {
      sendJson(res, {error: "Contract not found"}, 404)
      return
    }
    sendJson(res, contract)
    return
  }

  if (url.pathname === "/api/file") {
    const filePath = url.searchParams.get("path")
    if (filePath === null) {
      sendText(res, "Missing path", 400)
      return
    }
    const file = fileContents[filePath]
    if (file === undefined) {
      sendText(res, "File not found", 404)
      return
    }
    sendText(res, file)
    return
  }

  sendJson(res, {error: "Not found"}, 404)
})

server.listen(port, host, () => {
  console.log(`[acton-test-ui fixture-server] listening on http://${host}:${port}`)
})

const shutdown = (): void => {
  server.close(() => {
    // eslint-disable-next-line unicorn/no-process-exit -- e2e helper is a CLI entrypoint.
    process.exit(0)
  })
}

process.on("SIGINT", shutdown)
process.on("SIGTERM", shutdown)
