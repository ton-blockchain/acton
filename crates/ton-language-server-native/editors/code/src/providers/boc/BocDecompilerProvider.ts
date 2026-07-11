//  SPDX-License-Identifier: MIT
//  Copyright © 2025 TON Studio
import * as path from "node:path"

import * as vscode from "vscode"

import {Acton} from "../../acton/Acton"
import {DisasmCommand} from "../../acton/ActonCommand"

export class BocDecompilerProvider implements vscode.TextDocumentContentProvider {
  private readonly _onDidChange: vscode.EventEmitter<vscode.Uri> = new vscode.EventEmitter()
  public readonly onDidChange: vscode.Event<vscode.Uri> = this._onDidChange.event

  private readonly lastModified: Map<vscode.Uri, Date> = new Map()

  public static scheme: string = "boc-decompiled"

  public async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const bocUri = this.getBocPath(uri)

    try {
      return await this.disassembleBoc(bocUri)
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error)
      return this.formatError(errorMessage)
    }
  }

  private getBocPath(uri: vscode.Uri): vscode.Uri {
    console.log("Original URI:", uri.toString())
    const bocPath = uri.fsPath.replace(".decompiled.tasm", "")
    console.log("BoC path:", bocPath)
    return vscode.Uri.file(bocPath)
  }

  private async disassembleBoc(bocUri: vscode.Uri): Promise<string> {
    try {
      const command = new DisasmCommand(bocUri.fsPath)
      const result = await Acton.getInstance().spawn(command, path.dirname(bocUri.fsPath))
      if (result.exitCode !== 0) {
        const details = result.stderr.trim() || result.stdout.trim() || "unknown error"
        throw new Error(details)
      }

      return this.formatDisassembledOutput(result.stdout, bocUri)
    } catch (error: unknown) {
      const details = error instanceof Error ? error.message : String(error)
      throw new Error(`Disassembly failed: ${details}`)
    }
  }

  private formatDisassembledOutput(output: string, bocUri: vscode.Uri): string {
    const header = [
      "// Disassembled BoC file",
      "// Note: This is auto-generated code",
      `// Time: ${new Date().toISOString()}`,
      `// Source: ${bocUri.fsPath}`,
      "",
      "",
    ].join("\n")

    return header + output
  }

  private formatError(error: string): string {
    return [
      "// Failed to disassemble BoC file",
      "// Error: " + error,
      "// Time: " + new Date().toISOString(),
    ].join("\n")
  }

  public update(uri: vscode.Uri): void {
    const bocUri = this.getBocPath(uri)
    this.lastModified.set(bocUri, new Date())
    this._onDidChange.fire(uri)
  }
}
