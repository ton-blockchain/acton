import {spawn, type ChildProcess} from "node:child_process"
import fs from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import {fileURLToPath} from "node:url"

import {test as base, type Page} from "@playwright/test"

interface CommandResult {
  readonly stdout: string
  readonly stderr: string
}

class ProcessOutput {
  stdout = ""
  stderr = ""
}

interface FixtureProject {
  readonly tempDir: string
  readonly projectDir: string
  readonly homeDir: string
}

interface RunningActonUi {
  readonly baseUrl: string
  readonly projectDir: string
  readonly output: ProcessOutput
  readonly stop: () => Promise<void>
}

interface TestFixtures {
  readonly actonUi: RunningActonUi
}

interface WorkerFixtures {
  readonly startedActonUi: RunningActonUi
}

export interface VisualSnapshotOptions {
  readonly theme?: "light" | "dark"
}

const currentDir = path.dirname(fileURLToPath(import.meta.url))
const repositoryRoot = path.resolve(currentDir, "../../../..")
const actonBinary = process.env.ACTON_E2E_BIN ?? path.join(repositoryRoot, "target/debug/acton")
const tempParent = process.env.ACTON_E2E_TMPDIR ?? "/tmp"
const keepTemp = process.env.ACTON_E2E_KEEP_TEMP === "1"
const serverUrlPattern = /Starting\s+UI server at (http:\/\/127\.0\.0\.1:\d+)/
const startupTimeoutMs = 120_000
const shutdownTimeoutMs = 5000
const jettonSmokeFilter = [
  "deploy should create minter without bounce",
  "owner can send jettons",
  "transfer minimal value edge",
].join("|")

export const stabilizeVisualSnapshot = async (
  page: Page,
  options: VisualSnapshotOptions = {},
): Promise<void> => {
  await page.evaluate(async theme => {
    await document.fonts.ready

    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur()
    }

    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    localStorage.setItem("theme", theme)

    for (const element of document.querySelectorAll<HTMLElement>("[data-visual-dynamic]")) {
      const placeholder = element.dataset.visualPlaceholder ?? "<dynamic>"
      element.replaceChildren(document.createTextNode(placeholder))
      element.setAttribute("title", placeholder)
    }
  }, options.theme ?? "light")
}

const createFixtureProject = async (): Promise<FixtureProject> => {
  const tempDir = await fs.mkdtemp(path.join(tempParent, "acton-test-ui-e2e-"))
  const projectDir = path.join(tempDir, "jetton")
  const homeDir = path.join(tempDir, "home")
  await fs.mkdir(homeDir, {recursive: true})

  return {tempDir, projectDir, homeDir}
}

const actonEnv = (fixture: FixtureProject): NodeJS.ProcessEnv => ({
  ...process.env,
  ACTON_INTERNAL_SKIP_BROWSER: "1",
  ACTON_LOG_DIR: path.join(fixture.tempDir, "logs"),
  HOME: fixture.homeDir,
})

const formatCommandFailure = (
  command: string,
  args: readonly string[],
  result: CommandResult,
  reason: string,
): Error => {
  return new Error(
    [
      `${command} ${args.join(" ")} failed: ${reason}`,
      "",
      "stdout:",
      result.stdout.trimEnd(),
      "",
      "stderr:",
      result.stderr.trimEnd(),
    ].join("\n"),
  )
}

const runCommand = async (
  command: string,
  args: readonly string[],
  options: {readonly cwd: string; readonly env: NodeJS.ProcessEnv},
): Promise<CommandResult> => {
  const output = new ProcessOutput()

  await new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    })

    child.stdout?.on("data", (chunk: Buffer) => {
      output.stdout += chunk.toString()
    })
    child.stderr?.on("data", (chunk: Buffer) => {
      output.stderr += chunk.toString()
    })
    child.once("error", reject)
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve()
        return
      }

      reject(
        formatCommandFailure(
          command,
          args,
          output,
          `exit code ${code ?? "none"}, signal ${signal ?? "none"}`,
        ),
      )
    })
  })

  return output
}

const waitForHealth = async (baseUrl: string): Promise<void> => {
  const deadline = Date.now() + startupTimeoutMs
  let lastError = ""

  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseUrl}/api/health`, {cache: "no-store"})
      if (response.ok) {
        return
      }
      lastError = `HTTP ${response.status}`
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error)
    }

    await new Promise(resolve => setTimeout(resolve, 100))
  }

  throw new Error(`Timed out waiting for Test UI health at ${baseUrl}: ${lastError}`)
}

const waitForServerUrl = async (child: ChildProcess, output: ProcessOutput): Promise<string> => {
  return await new Promise<string>((resolve, reject) => {
    let settled = false

    const settle = (callback: () => void) => {
      if (settled) {
        return
      }
      settled = true
      clearTimeout(timer)
      callback()
    }

    const inspectStdout = () => {
      const match = output.stdout.match(serverUrlPattern)
      if (match?.[1]) {
        settle(() => resolve(match[1]))
      }
    }

    const timer = setTimeout(() => {
      settle(() => {
        reject(
          new Error(
            [
              "Timed out waiting for acton test --ui to print the Test UI URL.",
              "",
              "stdout:",
              output.stdout.trimEnd(),
              "",
              "stderr:",
              output.stderr.trimEnd(),
            ].join("\n"),
          ),
        )
      })
    }, startupTimeoutMs)

    child.stdout?.on("data", (chunk: Buffer) => {
      output.stdout += chunk.toString()
      inspectStdout()
    })
    child.stderr?.on("data", (chunk: Buffer) => {
      output.stderr += chunk.toString()
    })
    child.once("error", error => {
      settle(() => reject(error))
    })
    child.once("exit", (code, signal) => {
      settle(() => {
        reject(
          new Error(
            [
              `acton test --ui exited before the server became available: code ${code ?? "none"}, signal ${signal ?? "none"}`,
              "",
              "stdout:",
              output.stdout.trimEnd(),
              "",
              "stderr:",
              output.stderr.trimEnd(),
            ].join("\n"),
          ),
        )
      })
    })
  })
}

const stopProcess = async (child: ChildProcess): Promise<void> => {
  if (child.exitCode !== null || child.signalCode !== null) {
    return
  }

  await new Promise<void>(resolve => {
    const killTimer = setTimeout(() => {
      child.kill("SIGKILL")
    }, shutdownTimeoutMs)

    child.once("exit", () => {
      clearTimeout(killTimer)
      resolve()
    })
    child.kill()
  })
}

const startActonTestUi = async (): Promise<RunningActonUi> => {
  const fixture = await createFixtureProject()
  let child: ChildProcess | undefined

  try {
    await runCommand(
      actonBinary,
      ["new", fixture.projectDir, "--template", "jetton", "--name", "jetton-ui-e2e"],
      {cwd: repositoryRoot, env: actonEnv(fixture)},
    )

    const output = new ProcessOutput()
    child = spawn(
      actonBinary,
      ["test", "--ui", "--ui-port", "0", "--coverage", "--filter", jettonSmokeFilter],
      {
        cwd: fixture.projectDir,
        env: actonEnv(fixture),
        stdio: ["ignore", "pipe", "pipe"],
      },
    )

    const baseUrl = await waitForServerUrl(child, output)
    await waitForHealth(baseUrl)

    return {
      baseUrl,
      projectDir: fixture.projectDir,
      output,
      stop: async () => {
        if (child !== undefined) {
          await stopProcess(child)
        }
        if (!keepTemp) {
          await fs.rm(fixture.tempDir, {force: true, recursive: true})
        }
      },
    }
  } catch (error) {
    if (child !== undefined) {
      await stopProcess(child)
    }
    if (!keepTemp) {
      await fs.rm(fixture.tempDir, {force: true, recursive: true})
    }
    throw error
  }
}

export const test = base.extend<TestFixtures, WorkerFixtures>({
  startedActonUi: [
    // Playwright requires fixture functions to receive an object destructuring pattern.
    // eslint-disable-next-line no-empty-pattern
    async ({}, use) => {
      const running = await startActonTestUi()
      try {
        await use(running)
      } finally {
        await running.stop()
      }
    },
    {scope: "worker", timeout: startupTimeoutMs + shutdownTimeoutMs},
  ],

  actonUi: async ({startedActonUi}, use) => {
    await use(startedActonUi)
  },
})

export {expect} from "@playwright/test"
