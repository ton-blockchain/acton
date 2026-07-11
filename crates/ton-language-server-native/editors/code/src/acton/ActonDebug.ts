//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import type * as child_process from "node:child_process"

import * as vscode from "vscode"

import {Acton} from "./Acton"
import type {ActonCommand} from "./ActonCommand"
import {getFreeActonPort} from "./ActonPort"
import process from "node:process"

const DEBUG_HOST = "127.0.0.1"
const DEBUG_READY_TIMEOUT_MS = 120_000
const LEGACY_DAP_ENV = "ACTON_DEBUG_DAP_USE_LEGACY_VALUE"

interface StartActonDebuggingOptions {
  readonly connectionMode?: "hostPort" | "debugServer"
  readonly createCommand: (port: number) => ActonCommand
  readonly debugType?: string
  readonly outputChannelName: string
  readonly sessionName: string
  readonly waitForTermination?: boolean
  readonly workingDir: string
  readonly workspaceFolder?: vscode.WorkspaceFolder
}

export async function startActonDebugging(options: StartActonDebuggingOptions): Promise<void> {
  const port = await getFreeActonPort(DEBUG_HOST)
  const command = options.createCommand(port)
  const debugType = options.debugType ?? "acton"
  const connectionMode = options.connectionMode ?? "hostPort"
  let activeDebugSession: vscode.DebugSession | undefined
  let hasDebugSessionTerminated = false
  const actonProcess = await Acton.getInstance().spawnProcess(command, options.workingDir, {
    ...process.env,
    [LEGACY_DAP_ENV]: "1",
  })
  const terminal = createDebugTerminal(options.outputChannelName, () => {
    stopProcess(actonProcess)
    if (activeDebugSession && !hasDebugSessionTerminated) {
      void vscode.debug.stopDebugging(activeDebugSession)
    }
  })
  const handleProcessOutput = (chunk: Buffer): void => {
    terminal.append(chunk.toString())
  }

  actonProcess.stdout.on("data", handleProcessOutput)
  actonProcess.stderr.on("data", handleProcessOutput)
  actonProcess.once("close", code => {
    terminal.markProcessExited(code ?? undefined)
  })
  actonProcess.once("error", error => {
    terminal.appendLine(String(error))
  })

  terminal.show(true)
  terminal.appendLine(`> ${LEGACY_DAP_ENV}=1 acton ${renderArguments(command)}`)

  try {
    await waitForDebugServer(actonProcess, port, terminal)
  } catch (error) {
    stopProcess(actonProcess)
    terminal.show(true)
    throw error
  }

  const debugConfiguration: vscode.DebugConfiguration =
    connectionMode === "debugServer"
      ? {
          type: debugType,
          name: options.sessionName,
          request: "launch",
          debugServer: port,
        }
      : {
          type: debugType,
          name: options.sessionName,
          request: "launch",
          host: DEBUG_HOST,
          port,
        }

  let resolveTermination: (() => void) | undefined
  const terminationPromise: Promise<void> = new Promise(resolve => {
    resolveTermination = resolve
  })

  const startSubscription = vscode.debug.onDidStartDebugSession(session => {
    if (isOwnedDebugSession(session, debugConfiguration, port)) {
      activeDebugSession = session
    }
  })

  const terminateSubscription = vscode.debug.onDidTerminateDebugSession(session => {
    if (isOwnedDebugSession(session, debugConfiguration, port)) {
      startSubscription.dispose()
      terminateSubscription.dispose()
      activeDebugSession = undefined
      hasDebugSessionTerminated = true
      stopProcess(actonProcess)
      resolveTermination?.()
    }
  })

  let success = false
  try {
    terminal.appendLine(
      `Starting VS Code debug session (${debugType}, ${
        connectionMode === "debugServer" ? "debugServer" : "host/port"
      }) on ${DEBUG_HOST}:${port}`,
    )
    success = await vscode.debug.startDebugging(options.workspaceFolder, debugConfiguration)
  } catch (error) {
    startSubscription.dispose()
    terminateSubscription.dispose()
    stopProcess(actonProcess)
    terminal.show(true)
    throw error
  }

  if (!success) {
    startSubscription.dispose()
    terminateSubscription.dispose()
    stopProcess(actonProcess)
    terminal.show(true)
    throw new Error("Failed to start the Acton debug session.")
  }

  terminal.appendLine(`VS Code debug session started (${options.sessionName})`)

  if (options.waitForTermination) {
    await terminationPromise
  }
}

function renderArguments(command: ActonCommand): string {
  const args = command.getArguments()
  if (args.length === 0) {
    return command.name
  }

  return `${command.name} ${args.join(" ")}`
}

async function waitForDebugServer(
  actonProcess: child_process.ChildProcessWithoutNullStreams,
  port: number,
  terminal: DebugTerminal,
): Promise<void> {
  const readinessMarkers = [
    `Debugger server listening on ${DEBUG_HOST}:${port}`,
    `Retrace DAP listening on ${DEBUG_HOST}:${port}`,
    `Debugger listening on ${DEBUG_HOST}:${port}`,
  ]

  return new Promise((resolve, reject) => {
    let isSettled = false
    let transcript = ""
    const onData = (text: string): void => {
      append(text)
    }
    const dataSubscription = terminal.onDidWriteData(onData)

    const cleanup = (): void => {
      clearTimeout(timeout)
      dataSubscription.dispose()
      actonProcess.off("close", onClose)
      actonProcess.off("error", onError)
    }

    const finish = (callback: () => void): void => {
      if (isSettled) {
        return
      }

      isSettled = true
      cleanup()
      callback()
    }

    const append = (text: string): void => {
      transcript += text

      if (readinessMarkers.some(marker => transcript.includes(marker))) {
        finish(resolve)
      }
    }

    const onClose = (code: number | null): void => {
      finish(() => {
        reject(new Error(buildStartupFailureMessage(code, transcript)))
      })
    }

    const onError = (error: Error): void => {
      finish(() => {
        reject(error)
      })
    }

    const timeout = setTimeout(() => {
      finish(() => {
        reject(new Error(buildStartupTimeoutMessage(port, transcript)))
      })
    }, DEBUG_READY_TIMEOUT_MS)

    actonProcess.once("close", onClose)
    actonProcess.once("error", onError)
  })
}

function buildStartupFailureMessage(code: number | null, transcript: string): string {
  const exitCode = code === null ? "unknown" : String(code)
  const details = transcript.trim()

  if (details === "") {
    return `Acton debug failed before the debug adapter became ready (code=${exitCode}).`
  }

  return `Acton debug failed before the debug adapter became ready (code=${exitCode}).\n\n${details}`
}

function buildStartupTimeoutMessage(port: number, transcript: string): string {
  const details = transcript.trim()

  if (details === "") {
    return `Timed out waiting for Acton to listen on ${DEBUG_HOST}:${port}.`
  }

  return `Timed out waiting for Acton to listen on ${DEBUG_HOST}:${port}.\n\n${details}`
}

function stopProcess(actonProcess: child_process.ChildProcessWithoutNullStreams): void {
  if (actonProcess.killed || actonProcess.exitCode !== null) {
    return
  }

  actonProcess.kill()
}

function isOwnedDebugSession(
  session: vscode.DebugSession,
  debugConfiguration: vscode.DebugConfiguration,
  port: number,
): boolean {
  if (session.type !== debugConfiguration.type || session.name !== debugConfiguration.name) {
    return false
  }

  if (Number(session.configuration.debugServer) === port) {
    return true
  }

  return Number(session.configuration.port) === port
}

function createDebugTerminal(name: string, onClose?: () => void): DebugTerminal {
  return new DebugTerminal(name, onClose)
}

class DebugTerminal {
  private readonly dataEmitter: vscode.EventEmitter<string> = new vscode.EventEmitter<string>()
  private readonly writeEmitter: vscode.EventEmitter<string> = new vscode.EventEmitter<string>()
  private readonly terminal: vscode.Terminal
  private readonly pendingOutput: string[] = []
  private hasReportedProcessExit: boolean = false
  private isClosed: boolean = false
  private isOpen: boolean = false
  private lastWriteEndedWithCarriageReturn: boolean = false

  public constructor(
    name: string,
    private readonly onClose?: () => void,
  ) {
    this.terminal = vscode.window.createTerminal({
      name,
      pty: {
        close: () => {
          if (this.isClosed) {
            return
          }

          this.isClosed = true
          this.onClose?.()
          this.disposeEmitters()
        },
        onDidWrite: this.writeEmitter.event,
        open: () => {
          this.isOpen = true
          this.flushPendingOutput()
        },
      },
    })
  }

  public append(text: string): void {
    if (this.isClosed || text === "") {
      return
    }

    this.dataEmitter.fire(text)

    const normalized = this.normalizeForTerminal(text)
    if (normalized === "") {
      return
    }

    if (!this.isOpen) {
      this.pendingOutput.push(normalized)
      return
    }

    this.writeEmitter.fire(normalized)
  }

  public appendLine(text: string): void {
    this.append(`${text}\n`)
  }

  public markProcessExited(exitStatus?: number): void {
    if (this.isClosed || this.hasReportedProcessExit) {
      return
    }

    this.hasReportedProcessExit = true
    const detail =
      exitStatus === undefined
        ? "Acton debug process finished."
        : `Acton debug process exited with code ${exitStatus}.`
    this.appendLine(`\u001B[2m${detail}\u001B[0m`)
  }

  public onDidWriteData(listener: (text: string) => void): vscode.Disposable {
    return this.dataEmitter.event(listener)
  }

  public show(preserveFocus?: boolean): void {
    this.terminal.show(preserveFocus)
  }

  private disposeEmitters(): void {
    this.dataEmitter.dispose()
    this.writeEmitter.dispose()
  }

  private flushPendingOutput(): void {
    if (this.pendingOutput.length === 0) {
      return
    }

    this.writeEmitter.fire(this.pendingOutput.join(""))
    this.pendingOutput.length = 0
  }

  private normalizeForTerminal(text: string): string {
    let normalized = ""

    for (const char of text) {
      if (char === "\n") {
        normalized += this.lastWriteEndedWithCarriageReturn ? "\n" : "\r\n"
        this.lastWriteEndedWithCarriageReturn = false
        continue
      }

      normalized += char
      this.lastWriteEndedWithCarriageReturn = char === "\r"
    }

    return normalized
  }
}
