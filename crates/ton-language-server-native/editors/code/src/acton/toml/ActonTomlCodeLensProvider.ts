//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as path from "node:path"

import * as vscode from "vscode"

import {parseTomlTableHeaderPath} from "./acton-toml"

import {Acton} from "../Acton"
import {
  BuildCommand,
  CheckCommand,
  FormatCommand,
  InitCommand,
  RunCommand,
  TestCommand,
  WrapperCommand,
} from "../ActonCommand"
import {getFreeActonPort} from "../ActonPort"

export class ActonTomlCodeLensProvider implements vscode.CodeLensProvider {
  private readonly _onDidChangeCodeLenses: vscode.EventEmitter<void> =
    new vscode.EventEmitter<void>()
  public readonly onDidChangeCodeLenses: vscode.Event<void> = this._onDidChangeCodeLenses.event

  public refresh(): void {
    this._onDidChangeCodeLenses.fire()
  }

  public provideCodeLenses(
    document: vscode.TextDocument,
    _token: vscode.CancellationToken,
  ): vscode.CodeLens[] {
    if (!document.fileName.endsWith("Acton.toml")) {
      return []
    }

    const codeLenses: vscode.CodeLens[] = []
    const text = document.getText()
    const lines = text.split(/\r?\n/)

    for (const [i, raw_line] of lines.entries()) {
      const line = raw_line.trim()
      const currentSection = parseTomlTableHeaderPath(line)
      if (currentSection) {
        const section = currentSection.join(".")

        if (section === "test") {
          const range = new vscode.Range(i, 0, i, line.length)
          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "Run all tests",
              command: "ton.acton.testAll",
              arguments: [document.uri.fsPath],
            }),
            new vscode.CodeLens(range, {
              title: "with UI",
              command: "ton.acton.testAllUi",
              arguments: [document.uri.fsPath],
            }),
          )
        }

        if (section === "lint") {
          const range = new vscode.Range(i, 0, i, line.length)
          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "Check project",
              command: "ton.acton.checkProject",
              arguments: [document.uri.fsPath],
            }),
          )
        }

        if (section === "fmt") {
          const range = new vscode.Range(i, 0, i, line.length)
          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "Format project",
              command: "ton.acton.formatProject",
              arguments: [document.uri.fsPath],
            }),
          )
        }

        if (section === "wrappers.tolk") {
          const range = new vscode.Range(i, 0, i, line.length)
          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "Generate all Tolk wrappers",
              command: "ton.acton.generateAllTolkWrappers",
              arguments: [document.uri.fsPath],
            }),
          )
        }

        if (section === "wrappers.typescript") {
          const range = new vscode.Range(i, 0, i, line.length)
          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "Generate all TypeScript wrappers",
              command: "ton.acton.generateAllTypescriptWrappers",
              arguments: [document.uri.fsPath],
            }),
          )
        }

        if (currentSection.length === 2 && currentSection[0] === "contracts") {
          const contractId = currentSection[1]
          const range = new vscode.Range(i, 0, i, line.length)
          codeLenses.push(
            new vscode.CodeLens(range, {
              title: "Build contract",
              command: "ton.acton.buildContract",
              arguments: [document.uri.fsPath, contractId],
            }),
            new vscode.CodeLens(range, {
              title: "Retrace tx",
              command: "ton.acton.debugRetraceTransaction",
              arguments: [{tomlPath: document.uri.fsPath, contractId}],
            }),
          )
        }
      }
    }

    return codeLenses
  }

  public static registerCommands(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
      vscode.commands.registerCommand("ton.acton.testAll", async (tomlPath: string) => {
        const workingDir = path.dirname(tomlPath)
        await Acton.getInstance().execute(createProjectTestCommand(), workingDir)
      }),
      vscode.commands.registerCommand("ton.acton.testAllUi", async (tomlPath: string) => {
        const workingDir = path.dirname(tomlPath)
        const command = createProjectTestCommand()
        command.ui = true
        command.uiPort = String(await getFreeActonPort())
        await Acton.getInstance().execute(command, workingDir)
      }),
      vscode.commands.registerCommand("ton.acton.checkProject", async (tomlPath: string) => {
        const workingDir = path.dirname(tomlPath)
        await Acton.getInstance().execute(new CheckCommand(false), workingDir)
      }),
      vscode.commands.registerCommand("ton.acton.formatProject", async (tomlPath: string) => {
        const workingDir = path.dirname(tomlPath)
        await Acton.getInstance().execute(new FormatCommand(), workingDir)
      }),
      vscode.commands.registerCommand(
        "ton.acton.initDapp",
        async (resource?: vscode.Uri | string) => {
          const tomlPath = await resolveActonTomlPath(resource)
          if (!tomlPath) {
            await vscode.window.showWarningMessage(
              "Run this action from an Acton.toml context menu.",
            )
            return
          }

          const workingDir = path.dirname(tomlPath)
          await Acton.getInstance().execute(new InitCommand(true), workingDir)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.buildContract",
        async (tomlPath: string, contractId: string) => {
          const workingDir = path.dirname(tomlPath)
          await Acton.getInstance().execute(new BuildCommand(contractId), workingDir)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.runScript",
        async (tomlPath: string, scriptName: string) => {
          const workingDir = path.dirname(tomlPath)
          await Acton.getInstance().execute(new RunCommand(scriptName), workingDir)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.generateAllTolkWrappers",
        async (tomlPath: string) => {
          const workingDir = path.dirname(tomlPath)
          await Acton.getInstance().execute(new WrapperCommand("", false, true), workingDir)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.generateAllTypescriptWrappers",
        async (tomlPath: string) => {
          const workingDir = path.dirname(tomlPath)
          await Acton.getInstance().execute(new WrapperCommand("", true, true), workingDir)
        },
      ),
    )
  }
}

function createProjectTestCommand(): TestCommand {
  const command = new TestCommand()
  command.reporter = "console"
  return command
}

async function resolveActonTomlPath(resource?: vscode.Uri | string): Promise<string | undefined> {
  if (typeof resource === "string" && path.basename(resource) === "Acton.toml") {
    return resource
  }

  if (resource instanceof vscode.Uri && path.basename(resource.fsPath) === "Acton.toml") {
    return resource.fsPath
  }

  const activeDocument = vscode.window.activeTextEditor?.document
  if (activeDocument && path.basename(activeDocument.fileName) === "Acton.toml") {
    return activeDocument.fileName
  }

  const workspaceProject = await Acton.getInstance().findWorkspaceProject()
  return workspaceProject?.fsPath
}
