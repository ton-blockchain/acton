//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as vscode from "vscode"

import {parseTomlAssignmentKey, parseTomlTableHeaderPath} from "./acton-toml"

export class ActonTomlHoverProvider implements vscode.HoverProvider {
  public provideHover(
    document: vscode.TextDocument,
    position: vscode.Position,
    _token: vscode.CancellationToken,
  ): vscode.Hover | null {
    if (!document.fileName.endsWith("Acton.toml")) {
      return null
    }

    const line = document.lineAt(position.line).text
    const text = document.getText()
    const lines = text.split(/\r?\n/)

    let currentSection: string[] | null = null
    for (let i = 0; i <= position.line; i++) {
      const section = parseTomlTableHeaderPath(lines[i])
      if (section) {
        currentSection = section
      }
    }

    if (currentSection?.length !== 1 || currentSection[0] !== "scripts") {
      return null
    }

    const scriptKey = parseTomlAssignmentKey(line)
    if (scriptKey) {
      const scriptName = scriptKey.key

      if (position.character >= scriptKey.start && position.character <= scriptKey.end) {
        const args = [document.uri.fsPath, scriptName]
        const commandUri = vscode.Uri.parse(
          `command:ton.acton.runScript?${encodeURIComponent(JSON.stringify(args))}`,
        )
        const contents = new vscode.MarkdownString(`[Run Script](${commandUri.toString()})`)
        contents.isTrusted = true
        return new vscode.Hover(contents)
      }
    }

    return null
  }
}
