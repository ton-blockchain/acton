//  SPDX-License-Identifier: MIT
//  Copyright © 2025 TON Studio
import * as vscode from "vscode"

import {BocDecompilerProvider} from "../providers/boc/BocDecompilerProvider"

export function registerSaveBocDecompiledCommand(
  _context: vscode.ExtensionContext,
): vscode.Disposable {
  return vscode.commands.registerCommand(
    "ton.saveBocDecompiled",
    async (fileUri: vscode.Uri | undefined) => {
      try {
        await saveBoc(fileUri)
      } catch (error: unknown) {
        console.error("Error in saveBocDecompiledCommand:", error)
        const message = error instanceof Error ? error.message : String(error)
        vscode.window.showErrorMessage(`Failed to save disassembled BoC: ${message}`)
      }
    },
  )
}

export async function openBocFilePicker(): Promise<vscode.Uri | undefined> {
  const files = await vscode.window.showOpenDialog({
    canSelectFiles: true,
    canSelectFolders: false,
    canSelectMany: false,
    filters: {
      "BOC files": ["boc"],
    },
  })
  if (!files || files.length === 0) {
    return undefined
  }
  return files[0]
}

async function saveBoc(fileUri: vscode.Uri | undefined): Promise<void> {
  const actualFileUri = fileUri ?? (await openBocFilePicker())
  if (actualFileUri === undefined) return

  const decompiler = new BocDecompilerProvider()

  const decompileUri = actualFileUri.with({
    scheme: BocDecompilerProvider.scheme,
    path: actualFileUri.path + ".decompiled.tasm",
  })
  const content = await decompiler.provideTextDocumentContent(decompileUri)

  const outputPath = actualFileUri.fsPath + ".decompiled.tasm"

  const bytes = new TextEncoder().encode(content)
  vscode.workspace.fs.writeFile(vscode.Uri.file(outputPath), bytes)

  const relativePath = vscode.workspace.asRelativePath(outputPath)
  vscode.window.showInformationMessage(`Disassembled BOC saved to: ${relativePath}`)

  const savedFileUri = vscode.Uri.file(outputPath)
  const doc = await vscode.workspace.openTextDocument(savedFileUri)
  await vscode.window.showTextDocument(doc, {
    preview: false,
    viewColumn: vscode.ViewColumn.Active,
  })
}
