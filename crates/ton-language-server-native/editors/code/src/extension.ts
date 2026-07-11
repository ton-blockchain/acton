//  SPDX-License-Identifier: MIT
//  Copyright © 2025 TON Studio
import * as vscode from "vscode"
import type {FileSystemWatcher} from "vscode"

import {consoleError} from "./client-log"
import {restartLanguageServer, startLanguageServer, stopLanguageServer} from "./language-server"
import {LanguageServerProfileProvider} from "./language-server-profile-provider"

import {registerOpenBocCommand} from "./commands/openBocCommand"
import {BocEditorProvider} from "./providers/boc/BocEditorProvider"
import {BocFileSystemProvider} from "./providers/boc/BocFileSystemProvider"
import {BocDecompilerProvider} from "./providers/boc/BocDecompilerProvider"
import {registerSaveBocDecompiledCommand} from "./commands/saveBocDecompiledCommand"

import {WalletWebviewProvider} from "./providers/wallet/WalletWebviewProvider"

import {ActonTomlCodeLensProvider} from "./acton/toml/ActonTomlCodeLensProvider"
import {ActonTomlHoverProvider} from "./acton/toml/ActonTomlHoverProvider"
import {ActonTolkCodeLensProvider} from "./acton/tolk/ActonTolkCodeLensProvider"
import {ActonLinter} from "./acton/ActonLinter"
import {ActonTestController} from "./acton/ActonTestController"
import {registerActonRetraceDebugCommand} from "./acton/retrace/ActonRetraceDebug"
import {registerActonSetupNotifications} from "./acton/ActonSetup"
import {registerActonTerminalLinks} from "./acton/ActonTerminalLinks"
import {ActonAssemblyPreviewProvider} from "./acton/tolk/ActonAssemblyPreview"
import {configureDebugging} from "./debugging"

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  await checkConflictingExtensions()

  registerLanguageServerLifecycle(context)
  new LanguageServerProfileProvider().register(context)
  startLanguageServer(context).catch(consoleError)
  registerOpenBocCommand(context)
  registerSaveBocDecompiledCommand(context)
  registerActonSetupNotifications(context)
  registerActonTerminalLinks(context)

  const walletWebviewProvider = new WalletWebviewProvider(context.extensionUri)
  walletWebviewProvider.registerCommands(context)
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      WalletWebviewProvider.viewType,
      walletWebviewProvider,
    ),
  )

  // Acton integration
  const actonTolkCodeLensProvider = new ActonTolkCodeLensProvider()
  const actonTomlCodeLensProvider = new ActonTomlCodeLensProvider()
  const actonTomlHoverProvider = new ActonTomlHoverProvider()
  const actonTestController = new ActonTestController()
  const actonLinter = new ActonLinter()
  const actonAssemblyPreviewProvider = new ActonAssemblyPreviewProvider()
  actonAssemblyPreviewProvider.register(context)
  context.subscriptions.push(
    actonLinter,
    actonTestController,
    vscode.languages.registerCodeLensProvider({language: "tolk"}, actonTolkCodeLensProvider),
    vscode.languages.registerCodeLensProvider(
      {pattern: "**/Acton.toml"},
      actonTomlCodeLensProvider,
    ),
    vscode.languages.registerHoverProvider({pattern: "**/Acton.toml"}, actonTomlHoverProvider),
  )
  ActonTomlCodeLensProvider.registerCommands(context)
  ActonTolkCodeLensProvider.registerCommands(context)
  registerActonRetraceDebugCommand(context)

  configureDebugging(context)

  const config = vscode.workspace.getConfiguration("ton")
  const openDecompiled = config.get<boolean>("boc.openDecompiledOnOpen")
  if (openDecompiled) {
    BocEditorProvider.register()

    const bocFsProvider = new BocFileSystemProvider()
    context.subscriptions.push(
      vscode.workspace.registerFileSystemProvider("boc", bocFsProvider, {
        isCaseSensitive: true,
        isReadonly: false,
      }),
    )
  }

  const bocDecompilerProvider = new BocDecompilerProvider()
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(
      BocDecompilerProvider.scheme,
      bocDecompilerProvider,
    ),
  )

  const bocWatcher = registerBocWatcher(bocDecompilerProvider)
  context.subscriptions.push(bocWatcher)
}

const languageServerLaunchSettings = [
  "ton.acton.path",
  "ton.languageServer.args",
  "ton.languageServer.profiling.enabled",
  "ton.tolk.stdlib.path",
] as const

let restartPromptVisible = false

function registerLanguageServerLifecycle(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("ton.restartLanguageServer", async () => {
      try {
        await restartLanguageServer(context)
        void vscode.window.showInformationMessage("Acton language server restarted.")
      } catch (error) {
        consoleError("Failed to restart Acton language server", error)
        const message = error instanceof Error ? error.message : String(error)
        await vscode.window.showErrorMessage(`Failed to restart Acton language server: ${message}`)
        throw error
      }
    }),
    vscode.workspace.onDidChangeConfiguration(async event => {
      if (!languageServerLaunchSettings.some(setting => event.affectsConfiguration(setting))) {
        return
      }
      if (restartPromptVisible) {
        return
      }

      restartPromptVisible = true
      try {
        const action = await vscode.window.showInformationMessage(
          "Restart the Acton language server to apply the changed settings.",
          "Restart",
        )
        if (action === "Restart") {
          await vscode.commands.executeCommand("ton.restartLanguageServer")
        }
      } finally {
        restartPromptVisible = false
      }
    }),
  )
}

export function deactivate(): Thenable<void> | undefined {
  return stopLanguageServer()
}

function registerBocWatcher(bocDecompilerProvider: BocDecompilerProvider): FileSystemWatcher {
  const bocWatcher = vscode.workspace.createFileSystemWatcher("**/*.boc")

  bocWatcher.onDidChange((uri: vscode.Uri) => {
    const decompileUri = uri.with({
      scheme: BocDecompilerProvider.scheme,
      path: uri.path + ".decompiled.tasm",
    })

    const openDocument = vscode.workspace.textDocuments.find(
      doc => doc.uri.toString() === decompileUri.toString(),
    )

    if (openDocument) {
      bocDecompilerProvider.update(decompileUri)
    }
  })

  bocWatcher.onDidDelete((uri: vscode.Uri) => {
    const decompileUri = uri.with({
      scheme: BocDecompilerProvider.scheme,
      path: uri.path + ".decompiled.tasm",
    })

    const openDocument = vscode.workspace.textDocuments.find(
      doc => doc.uri.toString() === decompileUri.toString(),
    )

    if (openDocument) {
      bocDecompilerProvider.update(decompileUri)
    }
  })

  bocWatcher.onDidCreate((uri: vscode.Uri) => {
    const decompileUri = uri.with({
      scheme: BocDecompilerProvider.scheme,
      path: uri.path + ".decompiled.tasm",
    })

    const openDocument = vscode.workspace.textDocuments.find(
      doc => doc.uri.toString() === decompileUri.toString(),
    )

    if (openDocument) {
      bocDecompilerProvider.update(decompileUri)
    }
  })
  return bocWatcher
}

async function checkConflictingExtensions(): Promise<void> {
  const conflictingExtensions = [
    {id: "dotcypress.language-fift", name: "Fift"},
    {id: "tonwhales.func-vscode", name: "FunC Language Support"},
    {id: "raiym.func", name: "FunC"},
    {id: "natiiix.func-language-support", name: "FunC Language Support"},
    {id: "ton-core.tolk-vscode", name: "Tolk"},
  ]

  const installedConflicting = conflictingExtensions.filter(ext => {
    const extension = vscode.extensions.getExtension(ext.id)
    return extension?.isActive
  })

  if (installedConflicting.length === 0) {
    return
  }

  const extensionNames = installedConflicting.map(ext => ext.name).join(", ")
  const message = `Conflicting extensions detected: ${extensionNames}. We recommended to disable them to avoid conflicts. TON extension already includes the same functionality.`

  const action = await vscode.window.showWarningMessage(
    message,
    "Show conflicting extensions",
    "Ignore",
  )

  if (action === "Show conflicting extensions") {
    await vscode.commands.executeCommand("workbench.view.extensions")

    await vscode.commands.executeCommand(
      "workbench.extensions.search",
      `@id:${installedConflicting[0].id}`,
    )
  }
}
