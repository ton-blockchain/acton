import {spawn} from "node:child_process"
import {mkdtemp, readFile, rm} from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import {setTimeout as delay} from "node:timers/promises"

const port = process.env.ACTON_UI_E2E_NODE_PORT ?? "15411"
const executable = process.env.ACTON_E2E_BIN ?? path.resolve("target/debug/acton")
const directory = await mkdtemp("/tmp/acton-ui-snapshots-")
const node = spawn(
  executable,
  ["simulated-localnet", "start", "--port", port, "--no-mining", "--snapshots-dir", directory],
  {stdio: "inherit"},
)
const exited = new Promise<number>((resolve, reject) => {
  node.once("error", reject)
  node.once("exit", code => resolve(code ?? 0))
})
const stop = () => node.kill("SIGINT")
process.on("SIGINT", stop)
process.on("SIGTERM", stop)

try {
  const origin = `http://127.0.0.1:${port}`
  const deadline = Date.now() + 30_000

  let ready = false
  while (!ready) {
    const response = await fetch(`${origin}/acton_nodeInfo`).catch(() => undefined)
    ready = response?.ok ?? false
    if (Date.now() >= deadline) {
      throw new Error("Fixture node did not start")
    }

    if (!ready) {
      await delay(100)
    }
  }

  // Preserve u64/u128 JSON integers instead of rounding them through JavaScript numbers.
  const state = await readFile(new URL("./ui-state.json", import.meta.url), "utf8")
  const metadata = JSON.stringify({
    format_version: 1,
    id: "c56c6e30-e9e3-4cd3-9b57-ec19b0136e60",
    name: "Visual fixture",
    created_at: 0,
    fork_network: null,
  })
  const imported = await control(
    origin,
    "acton_importSnapshot",
    `${metadata.slice(0, -1)},"state":${state}}`,
  )
  await control(origin, "acton_restoreSnapshot", {id: imported.id})

  // Playwright waits for this line so no page reads the node before restoration finishes.
  process.stdout.write("Snapshot fixture ready\n")
  process.exitCode = await exited
} finally {
  stop()
  await exited
  await rm(directory, {recursive: true, force: true})
}

async function control(origin: string, method: string, payload: unknown) {
  const response = await fetch(`${origin}/${method}`, {
    method: "POST",
    headers: {"Content-Type": "application/json"},
    body: typeof payload === "string" ? payload : JSON.stringify(payload),
  })
  const body = await response.json()
  if (!response.ok || body.ok !== true) {
    throw new Error(`${method}: ${body.error}`)
  }

  return body.result
}
