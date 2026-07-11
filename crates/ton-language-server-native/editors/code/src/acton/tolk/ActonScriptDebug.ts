//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as path from "node:path"

import * as vscode from "vscode"

import {Acton} from "../Acton"
import {startActonDebugging} from "../ActonDebug"
import {ScriptCommand} from "../ActonCommand"

export async function startActonScriptDebugging(fileUri: vscode.Uri): Promise<void> {
  const tomlUri = await Acton.getInstance().findActonToml(fileUri)
  const workingDir = tomlUri ? path.dirname(tomlUri.fsPath) : path.dirname(fileUri.fsPath)
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(fileUri)

  await startActonDebugging({
    createCommand: port => {
      const command = new ScriptCommand(path.relative(workingDir, fileUri.fsPath))
      command.debug = true
      command.debugPort = String(port)
      return command
    },
    outputChannelName: "Acton Script Debug",
    sessionName: `Debug ${path.basename(fileUri.fsPath)}`,
    workingDir,
    workspaceFolder,
  })
}
