import type {ChildProcess} from "node:child_process"
import {spawn} from "node:child_process"
import {existsSync} from "node:fs"
import {createServer} from "node:net"
import path from "node:path"
import process from "node:process"

import {ActonError} from "./errors.js"
import type {StartLocalnetOptions} from "./types.js"

export type StartedLocalnetProcess = {
  readonly endpoint: string
  readonly child: ChildProcess
}

export async function startLocalnetProcess(
  options: StartLocalnetOptions,
): Promise<StartedLocalnetProcess> {
  if (options.requireAuth && !options.authToken) {
    throw new ActonError("Localnet.start({requireAuth: true}) requires authToken")
  }

  const port = options.port ?? (await findAvailablePort())
  const args = buildStartArgs(port, options)
  const projectRoot = resolveProjectRoot(options.projectRoot)
  const child = spawn(options.command ?? "acton", args, {
    cwd: projectRoot,
    env: {
      ...process.env,
      ...options.env,
      ...(options.requireAuth && options.authToken
        ? {ACTON_LOCALNET_AUTH_TOKEN: options.authToken}
        : {}),
    },
    stdio: options.stdio ?? "ignore",
  })

  return {
    child,
    endpoint: `http://127.0.0.1:${port}`,
  }
}

export function resolveProjectRoot(projectRoot: string | undefined): string {
  if (projectRoot !== undefined) {
    return projectRoot
  }

  return findProjectRoot(process.cwd())
}

export function buildStartArgs(port: number, options: StartLocalnetOptions): string[] {
  const args = ["localnet", "start", "--port", String(port)]

  pushOption(args, "--fork-net", options.forkNet)
  pushOption(args, "--fork-block-number", options.forkBlockNumber)
  if (options.accounts && options.accounts.length > 0) {
    pushOption(args, "--accounts", options.accounts.join(","))
  }
  pushOption(args, "--db-path", options.dbPath)
  pushOption(args, "--rate-limit", options.rateLimit)
  pushOption(args, "--response-delay-ms", options.responseDelayMs)
  pushOption(args, "--block-interval-ms", options.blockIntervalMs)
  pushFlag(args, "--no-mining", options.noMining)
  pushFlag(args, "--require-auth", options.requireAuth)
  pushOption(args, "--load-state", options.loadState)
  pushOption(args, "--dump-state", options.dumpState)

  return args
}

async function findAvailablePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer()
    server.once("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      server.close(() => {
        if (typeof address === "object" && address !== null) {
          resolve(address.port)
        } else {
          reject(new ActonError("Failed to allocate a local TCP port"))
        }
      })
    })
  })
}

function pushOption(args: string[], name: string, value: number | string | undefined): void {
  if (value !== undefined) {
    args.push(name, String(value))
  }
}

function pushFlag(args: string[], name: string, value: boolean | undefined): void {
  if (value) {
    args.push(name)
  }
}

function findProjectRoot(start: string): string {
  const initial = path.resolve(start)
  let current = initial

  while (true) {
    if (existsSync(path.join(current, "Acton.toml")) || existsSync(path.join(current, ".git"))) {
      return current
    }

    const parent = path.dirname(current)
    if (parent === current) {
      return initial
    }
    current = parent
  }
}
