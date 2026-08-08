import {mkdir, readFile, rm, writeFile} from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import {fileURLToPath} from "node:url"
import {runTests} from "@vscode/test-electron"

const extensionDevelopmentPath = path.resolve(fileURLToPath(new URL("..", import.meta.url)))
const extensionTestsPath = path.join(extensionDevelopmentPath, "e2e", "out", "index.js")
const workspacePath = path.join("/tmp", `acton-ls-vscode-e2e-${process.pid}`)
const userDataPath = path.join("/tmp", `acton-ls-vscode-user-${process.pid}`)
const extensionsPath = path.join("/tmp", `acton-ls-vscode-ext-${process.pid}`)
const serverLogPath = path.join("/tmp", `acton-ls-vscode-${process.pid}.log`)
const actonPath = path.resolve(
  process.env.ACTON_LS_E2E_BIN ??
    path.join(extensionDevelopmentPath, "../../../../target/debug/acton"),
)

async function createWorkspace(): Promise<void> {
  await mkdir(path.join(workspacePath, ".vscode"), {recursive: true})
  await mkdir(path.join(workspacePath, ".acton", "tolk-stdlib"), {recursive: true})

  await writeFile(
    path.join(workspacePath, "Acton.toml"),
    `[package]
name = "language-server-e2e"
description = "Language server extension smoke fixture"
version = "0.1.0"
license = "MIT"

[contracts.main]
display-name = "Main"
src = "main.tolk"
depends = []
`,
  )
  await writeFile(
    path.join(workspacePath, "lib.tolk"),
    `fun helper(): int { return 1 }
fun replacement(): int { return 2 }
`,
  )
  await writeFile(
    path.join(workspacePath, ".acton", "tolk-stdlib", "common.tolk"),
    "fun e2eStdlibMarker(): int { return 0 }\n",
  )
  await writeFile(
    path.join(workspacePath, "main.tolk"),
    `import "lib"

fun main(): int {
    return helper()
}
`,
  )
  await writeFile(
    path.join(workspacePath, ".vscode", "settings.json"),
    JSON.stringify(
      {
        "ton.acton.linter.enabled": false,
        "ton.acton.path": actonPath,
        "ton.acton.updateChecks.enabled": false,
        "ton.languageServer.args": [
          "ls",
          "--stdio",
          "--log-file",
          serverLogPath,
          "--log-level",
          "trace",
        ],
      },
      null,
      2,
    ),
  )
}

async function main(): Promise<void> {
  await createWorkspace()

  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      extensionTestsEnv: {ACTON_LS_E2E_BIN: actonPath},
      launchArgs: [
        workspacePath,
        "--disable-extensions",
        `--extensions-dir=${extensionsPath}`,
        `--user-data-dir=${userDataPath}`,
      ],
    })
  } catch (error) {
    try {
      console.error(await readFile(serverLogPath, "utf8"))
    } catch {
      console.error("Acton language server did not create its E2E log file")
    }
    throw error
  } finally {
    await Promise.all(
      [extensionsPath, userDataPath, workspacePath].map(directory =>
        rm(directory, {force: true, recursive: true}),
      ),
    )
    await rm(serverLogPath, {force: true})
  }
}

await main()
