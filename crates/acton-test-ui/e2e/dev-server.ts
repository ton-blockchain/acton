import {spawn, type ChildProcess} from "node:child_process"
import path from "node:path"
import {fileURLToPath} from "node:url"

const directoryName = path.dirname(fileURLToPath(import.meta.url))
const packageRoot = path.resolve(directoryName, "..")
const apiPort = process.env.ACTON_TEST_UI_API_PORT ?? "4310"
const uiPort = process.env.ACTON_TEST_UI_E2E_PORT ?? "4173"
const apiTarget = `http://127.0.0.1:${apiPort}`

const children: ChildProcess[] = []

function spawnChild(command: string, args: string[], extraEnv: NodeJS.ProcessEnv = {}): ChildProcess {
  const child = spawn(command, args, {
    cwd: packageRoot,
    stdio: "inherit",
    env: {
      ...process.env,
      ...extraEnv,
    },
  })

  child.on("exit", code => {
    if (code !== 0) {
      shutdown(code ?? 1)
    }
  })

  children.push(child)
  return child
}

function shutdown(exitCode = 0): void {
  for (const child of children) {
    if (!child.killed) {
      child.kill("SIGTERM")
    }
  }
  // eslint-disable-next-line unicorn/no-process-exit -- e2e helper is a CLI entrypoint.
  process.exit(exitCode)
}

process.on("SIGINT", () => shutdown(0))
process.on("SIGTERM", () => shutdown(0))

spawnChild("bun", ["./e2e/fixture-server.ts"], {
  ACTON_TEST_UI_API_PORT: apiPort,
})

spawnChild("bun", ["x", "vite", "--host", "127.0.0.1", "--port", uiPort, "--strictPort"], {
  ACTON_TEST_UI_API_PROXY_TARGET: apiTarget,
})
