//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as fs from "node:fs/promises"
import * as os from "node:os"
import * as path from "node:path"

import * as vscode from "vscode"

import {consoleError} from "../client-log"

import {Acton} from "./Acton"
import {FormatCommand} from "./ActonCommand"

function isActonUnavailableError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error && error.code === "ENOENT"
}

function fullDocumentRange(document: vscode.TextDocument): vscode.Range {
  const lastLine = document.lineCount - 1
  const end = document.lineAt(lastLine).range.end
  return new vscode.Range(new vscode.Position(0, 0), end)
}

function byteColumn(document: vscode.TextDocument, position: vscode.Position): number {
  const linePrefix = document.lineAt(position.line).text.slice(0, position.character)
  return Buffer.byteLength(linePrefix, "utf8")
}

function formatActonRange(document: vscode.TextDocument, range: vscode.Range): string {
  const validRange = document.validateRange(range)
  const startColumn = byteColumn(document, validRange.start)
  const endColumn = byteColumn(document, validRange.end)
  return `${validRange.start.line}:${startColumn}-${validRange.end.line}:${endColumn}`
}

async function resolveWorkingDirectory(document: vscode.TextDocument): Promise<string | undefined> {
  if (document.uri.scheme === "file") {
    const actonToml = await Acton.getInstance().findActonToml(document.uri)
    if (actonToml) {
      return path.dirname(actonToml.fsPath)
    }

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri)
    if (workspaceFolder) {
      return workspaceFolder.uri.fsPath
    }
  }

  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath
}

export async function formatTolkDocumentWithActon(
  document: vscode.TextDocument,
  range?: vscode.Range,
): Promise<vscode.TextEdit[] | null> {
  const formatterEnabled = vscode.workspace
    .getConfiguration("ton", document.uri)
    .get<boolean>("tolk.formatter.useFormatter", true)
  if (!formatterEnabled) {
    return []
  }

  const source = document.getText()
  const workingDirectory = await resolveWorkingDirectory(document)
  const tempDirectory = await fs.mkdtemp(path.join(os.tmpdir(), "acton-fmt-"))

  try {
    const originalFileName =
      document.uri.scheme === "file" ? path.basename(document.uri.fsPath) : "untitled.tolk"
    const tempFilePath = path.join(tempDirectory, originalFileName)
    await fs.writeFile(tempFilePath, source, "utf8")

    const command = new FormatCommand(
      [tempFilePath],
      range ? formatActonRange(document, range) : "",
    )
    const {exitCode, stderr, stdout} = await Acton.getInstance().spawn(command, workingDirectory)

    if (exitCode !== 0) {
      const details = (stderr.trim() || stdout.trim() || `exit code ${exitCode}`).split("\n")[0]
      void vscode.window.showErrorMessage(`Failed to format with acton fmt: ${details}`)
      return []
    }

    const formatted = await fs.readFile(tempFilePath, "utf8")
    if (formatted === source) {
      return []
    }

    return [vscode.TextEdit.replace(fullDocumentRange(document), formatted)]
  } catch (error) {
    if (isActonUnavailableError(error)) {
      return null
    }

    consoleError("Failed to format with acton fmt", error)
    void vscode.window.showErrorMessage("Failed to format with acton fmt")
    return []
  } finally {
    await fs.rm(tempDirectory, {recursive: true, force: true}).catch(() => {
      // ignore cleanup errors for temporary files
    })
  }
}
