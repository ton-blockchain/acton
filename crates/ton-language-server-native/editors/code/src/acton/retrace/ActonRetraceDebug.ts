//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as path from "node:path"

import * as vscode from "vscode"

import {Acton} from "../Acton"
import {startActonDebugging} from "../ActonDebug"
import {RetraceCommand} from "../ActonCommand"

import {normalizeRetraceHash, parseActonContractIds} from "./ActonRetraceUtils"

const RETRACE_NETWORKS = [
  {
    label: "Auto",
    description: "Try mainnet first, then fall back to testnet",
    network: "",
  },
  {
    label: "Mainnet",
    description: "Retrace only on TON mainnet",
    network: "mainnet",
  },
  {
    label: "Testnet",
    description: "Retrace only on TON testnet",
    network: "testnet",
  },
] as const

interface DebugRetraceCommandArgs {
  readonly tomlPath?: string
  readonly contractId?: string
}

interface ActonProjectContext {
  readonly tomlUri: vscode.Uri
  readonly workingDir: string
  readonly workspaceFolder: vscode.WorkspaceFolder | undefined
}

export function registerActonRetraceDebugCommand(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "ton.acton.debugRetraceTransaction",
      async (args?: DebugRetraceCommandArgs) => {
        try {
          await startActonRetraceDebugging(args)
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error)
          void vscode.window.showErrorMessage(message)
        }
      },
    ),
  )
}

async function startActonRetraceDebugging(args?: DebugRetraceCommandArgs): Promise<void> {
  const project = await resolveActonProjectContext(args)
  if (!project) {
    return
  }

  const contractIds = await loadActonContractIds(project.tomlUri)
  if (contractIds.length === 0) {
    throw new Error(`No contracts were found in ${path.basename(project.tomlUri.fsPath)}.`)
  }

  const contractId = await resolveContractId(args?.contractId, contractIds)
  if (!contractId) {
    return
  }

  const hash = await promptForRetraceHash()
  if (!hash) {
    return
  }

  const network = await promptForRetraceNetwork()
  if (network === undefined) {
    return
  }

  await startActonDebugging({
    connectionMode: "debugServer",
    createCommand: port => {
      const command = new RetraceCommand(hash)
      command.contractId = contractId
      command.net = network
      command.debug = true
      command.debugPort = String(port)
      return command
    },
    debugType: "tolk",
    outputChannelName: "Acton Retrace Debug",
    sessionName: `Retrace ${contractId} ${hash.slice(0, 8)}`,
    workingDir: project.workingDir,
    workspaceFolder: project.workspaceFolder,
  })
}

async function resolveActonProjectContext(
  args?: DebugRetraceCommandArgs,
): Promise<ActonProjectContext | undefined> {
  const explicitTomlUri = args?.tomlPath ? vscode.Uri.file(args.tomlPath) : undefined
  const startUri =
    explicitTomlUri ??
    vscode.window.activeTextEditor?.document.uri ??
    vscode.workspace.workspaceFolders?.[0]?.uri

  if (!startUri) {
    void vscode.window.showErrorMessage(
      "Open an Acton project or an Acton.toml file before starting retrace debugging.",
    )
    return undefined
  }

  const tomlUri = explicitTomlUri ?? (await Acton.getInstance().findActonToml(startUri))
  if (!tomlUri) {
    void vscode.window.showErrorMessage(
      "Could not find Acton.toml. Retrace debugging requires an Acton project.",
    )
    return undefined
  }

  return {
    tomlUri,
    workingDir: path.dirname(tomlUri.fsPath),
    workspaceFolder:
      vscode.workspace.getWorkspaceFolder(tomlUri) ?? vscode.workspace.getWorkspaceFolder(startUri),
  }
}

async function loadActonContractIds(tomlUri: vscode.Uri): Promise<string[]> {
  const content = await vscode.workspace.fs.readFile(tomlUri)
  return parseActonContractIds(Buffer.from(content).toString("utf8"))
}

async function resolveContractId(
  preselectedContractId: string | undefined,
  contractIds: readonly string[],
): Promise<string | undefined> {
  if (preselectedContractId) {
    if (contractIds.includes(preselectedContractId)) {
      return preselectedContractId
    }

    throw new Error(`Contract "${preselectedContractId}" was not found in Acton.toml.`)
  }

  if (contractIds.length === 1) {
    return contractIds[0]
  }

  const items = contractIds.map(contractId => ({
    label: contractId,
    description: "Contract from Acton.toml",
  }))
  const selected = await vscode.window.showQuickPick(items, {
    placeHolder: "Select the contract to use for source-level retrace debugging",
    canPickMany: false,
  })

  return selected?.label
}

async function promptForRetraceHash(): Promise<string | undefined> {
  const input = await vscode.window.showInputBox({
    prompt: "Enter the transaction hash to retrace",
    placeHolder: "64-character hex hash",
    ignoreFocusOut: true,
    validateInput: value => {
      return normalizeRetraceHash(value)
        ? undefined
        : "Enter a 64-character transaction hash in hex format."
    },
  })

  if (!input) {
    return undefined
  }

  return normalizeRetraceHash(input) ?? undefined
}

async function promptForRetraceNetwork(): Promise<string | undefined> {
  const selected = await vscode.window.showQuickPick(RETRACE_NETWORKS, {
    placeHolder: "Select the network for acton retrace",
    canPickMany: false,
  })

  return selected?.network
}
