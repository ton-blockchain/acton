//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import type {ChildProcessWithoutNullStreams} from "node:child_process"

import * as vscode from "vscode"

import {consoleError} from "../client-log"

import {Acton} from "./Acton"
import {CheckCommand} from "./ActonCommand"

const APPLY_FIX_AND_SAVE_COMMAND = "ton.acton.applyFixAndSave"

interface ActonRange {
  readonly start: {readonly line: number; readonly character: number}
  readonly end: {readonly line: number; readonly character: number}
}

interface ActonAnnotation {
  readonly range: ActonRange
  readonly message?: string
  readonly is_primary: boolean
  readonly tags?: string[]
}

interface ActonEdit {
  readonly range: ActonRange
  readonly newText: string
  readonly file: string
}

interface ActonFix {
  readonly message: string
  readonly edits: readonly ActonEdit[]
  readonly applicability: "auto" | "manual"
}

interface ActonDiagnostic {
  readonly file: string
  readonly severity: "error" | "warning" | "info"
  readonly name: string
  readonly code?: string
  readonly message: string
  readonly annotations: readonly ActonAnnotation[]
  readonly fixes: readonly ActonFix[]
  readonly source: string
}

interface ActonCheckOutput {
  readonly success: boolean
  readonly diagnostics: readonly ActonDiagnostic[]
}

export class ActonLinter implements vscode.CodeActionProvider {
  private readonly diagnosticCollection: vscode.DiagnosticCollection
  private debounceTimer: NodeJS.Timeout | undefined
  private checkRequestId: number = 0
  private readonly disposables: vscode.Disposable[] = []
  private latestDiagnostics: readonly ActonDiagnostic[] = []
  private currentCheckProcess: ChildProcessWithoutNullStreams | undefined
  private currentCheckUri: vscode.Uri | undefined

  public constructor() {
    this.diagnosticCollection = vscode.languages.createDiagnosticCollection("acton")

    this.disposables.push(
      vscode.commands.registerCommand(
        APPLY_FIX_AND_SAVE_COMMAND,
        async (filePaths: readonly string[] | undefined) => {
          await this.saveEditedFiles(filePaths)
        },
      ),
      vscode.languages.registerCodeActionsProvider("tolk", this, {
        providedCodeActionKinds: [vscode.CodeActionKind.QuickFix],
      }),
      vscode.workspace.onDidChangeTextDocument(e => {
        if (e.document.languageId !== "tolk") {
          return
        }
        if (e.document.uri.scheme !== "file") {
          return
        }
        if (e.contentChanges.length === 0) {
          return
        }

        this.checkRequestId += 1
        this.cancelRunningCheck()
      }),
      vscode.window.onDidChangeActiveTextEditor(editor => {
        if (editor?.document.languageId === "tolk") {
          this.triggerCheck(editor.document)
        }
      }),
      vscode.workspace.onDidSaveTextDocument(doc => {
        if (doc.languageId === "tolk") {
          this.triggerCheck(doc)
        }
      }),
      vscode.workspace.onDidCloseTextDocument(doc => {
        if (this.currentCheckUri?.toString() === doc.uri.toString()) {
          this.checkRequestId += 1
          this.cancelRunningCheck()
        }
        this.clearDiagnosticsForUri(doc.uri)
      }),
      vscode.workspace.onDidChangeConfiguration(e => {
        if (e.affectsConfiguration("ton.acton.linter")) {
          this.triggerCheck()
        }
      }),
    )

    this.triggerCheck()
  }

  public dispose(): void {
    this.checkRequestId += 1
    this.cancelRunningCheck()
    this.clearAllDiagnostics()
    this.diagnosticCollection.dispose()
    for (const disposable of this.disposables) {
      disposable.dispose()
    }
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer)
    }
  }

  private triggerCheck(document?: vscode.TextDocument): void {
    const config = vscode.workspace.getConfiguration("ton.acton.linter")
    const enabled = config.get<boolean>("enabled", true)

    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer)
      this.debounceTimer = undefined
    }

    if (!enabled) {
      this.checkRequestId += 1
      this.cancelRunningCheck()
      this.clearAllDiagnostics()
      return
    }

    const targetDocument = document ?? vscode.window.activeTextEditor?.document
    if (targetDocument?.languageId !== "tolk" || targetDocument.uri.scheme !== "file") {
      return
    }

    if (targetDocument.isDirty) {
      this.checkRequestId += 1
      this.cancelRunningCheck()
      return
    }

    const debounceMs = Math.max(0, config.get<number>("debounce", 500))
    const requestId = ++this.checkRequestId
    const targetUri = targetDocument.uri
    this.cancelRunningCheck()

    this.debounceTimer = setTimeout(() => {
      this.debounceTimer = undefined
      void this.runCheck(requestId, targetUri)
    }, debounceMs)
  }

  private async runCheck(requestId: number, targetUri: vscode.Uri): Promise<void> {
    if (requestId !== this.checkRequestId) {
      return
    }

    const acton = Acton.getInstance()
    const tomlUri = await acton.findActonToml(targetUri)
    if (requestId !== this.checkRequestId) {
      return
    }
    if (!tomlUri) {
      this.clearDiagnosticsForUri(targetUri)
      return
    }

    const workingDirectory = vscode.Uri.joinPath(tomlUri, "..").fsPath
    const command = new CheckCommand(true, targetUri.fsPath)
    const child = await acton.spawnProcess(command, workingDirectory)
    if (requestId !== this.checkRequestId) {
      child.kill()
      return
    }
    this.currentCheckProcess = child
    this.currentCheckUri = targetUri

    try {
      const {stdout, stderr} = await this.collectProcessOutput(child)
      if (this.currentCheckProcess === child) {
        this.currentCheckProcess = undefined
        this.currentCheckUri = undefined
      }
      if (requestId !== this.checkRequestId) {
        return
      }

      if (stdout.trim() === "") {
        this.clearDiagnosticsForUri(targetUri)
        return
      }

      let output: ActonCheckOutput
      try {
        output = JSON.parse(stdout) as ActonCheckOutput
      } catch (error) {
        consoleError("Failed to parse acton check output", error, stdout, stderr)
        this.clearDiagnosticsForUri(targetUri)
        return
      }

      if (requestId !== this.checkRequestId) {
        return
      }

      this.latestDiagnostics = output.diagnostics
      this.processDiagnostics(output)
    } catch (error) {
      if (this.currentCheckProcess === child) {
        this.currentCheckProcess = undefined
        this.currentCheckUri = undefined
      }
      if (requestId === this.checkRequestId) {
        this.clearDiagnosticsForUri(targetUri)
        consoleError("Failed to run acton check", error)
      }
    }
  }

  private async collectProcessOutput(
    child: ChildProcessWithoutNullStreams,
  ): Promise<{stdout: string; stderr: string}> {
    return new Promise((resolve, reject) => {
      let stdout = ""
      let stderr = ""
      let settled = false

      child.stdout.on("data", (data: Buffer) => {
        stdout += data.toString()
      })

      child.stderr.on("data", (data: Buffer) => {
        stderr += data.toString()
      })

      child.on("close", () => {
        if (settled) return
        settled = true
        resolve({stdout, stderr})
      })

      child.on("error", (err: Error) => {
        if (settled) return
        settled = true
        reject(err)
      })
    })
  }

  private cancelRunningCheck(): void {
    if (!this.currentCheckProcess) {
      return
    }

    try {
      this.currentCheckProcess.kill()
    } catch {
      // ignore cancellation errors
    }

    this.currentCheckProcess = undefined
    this.currentCheckUri = undefined
  }

  private clearDiagnosticsForUri(uri: vscode.Uri): void {
    const uriString = uri.toString()
    this.latestDiagnostics = this.latestDiagnostics.filter(diag => {
      try {
        return vscode.Uri.file(diag.file).toString() !== uriString
      } catch {
        return true
      }
    })
    this.diagnosticCollection.delete(uri)
  }

  private clearAllDiagnostics(): void {
    this.latestDiagnostics = []
    this.diagnosticCollection.clear()
  }

  public provideCodeActions(
    document: vscode.TextDocument,
    _range: vscode.Range | vscode.Selection,
    context: vscode.CodeActionContext,
  ): vscode.CodeAction[] {
    const actions: vscode.CodeAction[] = []

    for (const diagnostic of context.diagnostics) {
      if (diagnostic.source !== "acton" || !diagnostic.code) {
        continue
      }

      // Find the original acton diagnostic
      const actonDiag = this.latestDiagnostics.find(d =>
        this.matchesDiagnostic(document.uri, diagnostic, d),
      )

      if (actonDiag?.fixes) {
        for (const fix of actonDiag.fixes) {
          const action = new vscode.CodeAction(fix.message, vscode.CodeActionKind.QuickFix)
          action.diagnostics = [diagnostic]
          action.edit = new vscode.WorkspaceEdit()

          for (const edit of fix.edits) {
            const editRange = new vscode.Range(
              edit.range.start.line,
              edit.range.start.character,
              edit.range.end.line,
              edit.range.end.character,
            )
            action.edit.replace(vscode.Uri.file(edit.file), editRange, edit.newText)
          }

          if (fix.applicability === "auto") {
            action.isPreferred = true
          }

          const editedFiles = [...new Set(fix.edits.map(edit => edit.file))]
          action.command = {
            title: "Save Acton quick fix files",
            command: APPLY_FIX_AND_SAVE_COMMAND,
            arguments: [editedFiles],
          }

          actions.push(action)
        }
      }
    }

    return actions
  }

  private async saveEditedFiles(filePaths: readonly string[] | undefined): Promise<void> {
    if (!filePaths || filePaths.length === 0) {
      return
    }

    const editedUris = new Set(filePaths.map(filePath => vscode.Uri.file(filePath).toString()))

    for (const filePath of filePaths) {
      const uri = vscode.Uri.file(filePath)

      try {
        const document =
          vscode.workspace.textDocuments.find(doc => doc.uri.toString() === uri.toString()) ??
          (await vscode.workspace.openTextDocument(uri))

        if (!document.isDirty) {
          continue
        }

        const saved = await document.save()
        if (!saved) {
          consoleError("Failed to save Acton quick fix file", filePath)
        }
      } catch (error) {
        consoleError("Failed to save Acton quick fix file", filePath, error)
      }
    }

    const activeDocument = vscode.window.activeTextEditor?.document
    if (
      activeDocument &&
      editedUris.has(activeDocument.uri.toString()) &&
      !activeDocument.isDirty
    ) {
      this.triggerCheck(activeDocument)
    }
  }

  private processDiagnostics(output: ActonCheckOutput): void {
    const diagnosticsByFile: Map<string, vscode.Diagnostic[]> = new Map()

    for (const diag of output.diagnostics) {
      const severity = this.mapSeverity(diag.severity)
      const primaryAnnotation = diag.annotations.find(a => a.is_primary) ?? diag.annotations.at(0)

      if (!primaryAnnotation) continue

      const range = new vscode.Range(
        primaryAnnotation.range.start.line,
        primaryAnnotation.range.start.character,
        primaryAnnotation.range.end.line,
        primaryAnnotation.range.end.character,
      )

      const message = diag.code ? `[${diag.code}] ${diag.message}` : diag.message
      const diagnostic = new vscode.Diagnostic(range, message, severity)
      diagnostic.source = "acton"
      diagnostic.code = diag.code

      // Tags
      const tags: vscode.DiagnosticTag[] = []
      for (const annotation of diag.annotations) {
        for (const tag of annotation.tags ?? []) {
          switch (tag.toLowerCase()) {
            case "unnecessary": {
              tags.push(vscode.DiagnosticTag.Unnecessary)
              break
            }
            case "deprecated": {
              tags.push(vscode.DiagnosticTag.Deprecated)
              break
            }
          }
        }
      }
      if (tags.length > 0) {
        diagnostic.tags = tags
      }

      // Related information
      const relatedInformation: vscode.DiagnosticRelatedInformation[] = []
      for (const annotation of diag.annotations) {
        if (annotation === primaryAnnotation && !annotation.message) continue

        const relatedRange = new vscode.Range(
          annotation.range.start.line,
          annotation.range.start.character,
          annotation.range.end.line,
          annotation.range.end.character,
        )

        relatedInformation.push(
          new vscode.DiagnosticRelatedInformation(
            new vscode.Location(vscode.Uri.file(diag.file), relatedRange),
            annotation.message ?? (annotation.is_primary ? "here" : "related context"),
          ),
        )
      }
      if (relatedInformation.length > 0) {
        diagnostic.relatedInformation = relatedInformation
      }

      const fileDiagnostics = diagnosticsByFile.get(diag.file) ?? []
      fileDiagnostics.push(diagnostic)
      diagnosticsByFile.set(diag.file, fileDiagnostics)
    }

    // Update diagnostic collection
    this.diagnosticCollection.clear()
    for (const [file, fileDiagnostics] of diagnosticsByFile) {
      this.diagnosticCollection.set(vscode.Uri.file(file), fileDiagnostics)
    }
  }

  private mapSeverity(severity: string): vscode.DiagnosticSeverity {
    switch (severity) {
      case "error": {
        return vscode.DiagnosticSeverity.Error
      }
      case "warning": {
        return vscode.DiagnosticSeverity.Warning
      }
      case "info": {
        return vscode.DiagnosticSeverity.Information
      }
      default: {
        return vscode.DiagnosticSeverity.Information
      }
    }
  }

  private matchesDiagnostic(
    documentUri: vscode.Uri,
    diagnostic: vscode.Diagnostic,
    actonDiagnostic: ActonDiagnostic,
  ): boolean {
    const message = actonDiagnostic.code
      ? `[${actonDiagnostic.code}] ${actonDiagnostic.message}`
      : actonDiagnostic.message
    if (actonDiagnostic.code !== (diagnostic.code as string) || message !== diagnostic.message) {
      return false
    }

    try {
      if (vscode.Uri.file(actonDiagnostic.file).toString() !== documentUri.toString()) {
        return false
      }
    } catch {
      return false
    }

    const primaryAnnotation =
      actonDiagnostic.annotations.find(a => a.is_primary) ?? actonDiagnostic.annotations.at(0)
    if (!primaryAnnotation) {
      return false
    }

    return this.rangesEqual(primaryAnnotation.range, diagnostic.range)
  }

  private rangesEqual(range: ActonRange, diagnosticRange: vscode.Range): boolean {
    return (
      range.start.line === diagnosticRange.start.line &&
      range.start.character === diagnosticRange.start.character &&
      range.end.line === diagnosticRange.end.line &&
      range.end.character === diagnosticRange.end.character
    )
  }
}
