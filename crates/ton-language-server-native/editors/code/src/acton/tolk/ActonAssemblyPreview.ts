//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as fs from "node:fs/promises"
import * as os from "node:os"
import * as path from "node:path"

import * as vscode from "vscode"

import {Acton, type ActonSpawnResult} from "../Acton"
import {CompileCommand, DisasmCommand} from "../ActonCommand"
import {stripAnsiCodes} from "../ansi"

interface TolkCompileJsonResult {
  readonly success: boolean
  readonly code_boc64?: string
  readonly error?: string
}

interface TolkDisasmJsonResult {
  readonly success: boolean
  readonly assembly?: string
  readonly blocks?: readonly TolkDisasmJsonBlock[]
  readonly error?: string
}

interface TolkDisasmJsonBlock {
  readonly source?: TolkDisasmJsonSourceLocation
  readonly assembly_ranges?: readonly TolkDisasmJsonRange[]
}

interface TolkDisasmJsonSourceLocation {
  readonly file: string
  readonly line: number
  readonly end_line: number
}

interface TolkDisasmJsonRange {
  readonly start_line: number
  readonly end_line: number
}

interface AssemblyLineRange {
  readonly startLine: number
  readonly endLine: number
}

interface AssemblyPreviewBlock {
  readonly sourceLine: number
  readonly paletteIndex: number
  readonly assemblyRanges: readonly AssemblyLineRange[]
}

interface AssemblyPreviewState {
  readonly sourceUri: vscode.Uri
  readonly previewUri: vscode.Uri
  readonly blocks: readonly AssemblyPreviewBlock[]
}

const PREVIEW_SUFFIX = ".assembly.tasm"
const BLOCK_PALETTE: readonly {
  readonly weakLight: string
  readonly weakDark: string
  readonly activeLight: string
  readonly activeDark: string
}[] = [
  {
    weakLight: "rgba(75, 130, 220, 0.07)",
    weakDark: "rgba(105, 170, 255, 0.10)",
    activeLight: "rgba(60, 120, 220, 0.26)",
    activeDark: "rgba(105, 170, 255, 0.32)",
  },
  {
    weakLight: "rgba(80, 160, 85, 0.07)",
    weakDark: "rgba(100, 190, 120, 0.10)",
    activeLight: "rgba(70, 150, 80, 0.25)",
    activeDark: "rgba(100, 190, 120, 0.30)",
  },
  {
    weakLight: "rgba(215, 135, 45, 0.07)",
    weakDark: "rgba(245, 170, 85, 0.10)",
    activeLight: "rgba(210, 125, 35, 0.25)",
    activeDark: "rgba(245, 170, 85, 0.30)",
  },
  {
    weakLight: "rgba(145, 95, 215, 0.07)",
    weakDark: "rgba(185, 140, 255, 0.10)",
    activeLight: "rgba(135, 85, 210, 0.25)",
    activeDark: "rgba(185, 140, 255, 0.31)",
  },
  {
    weakLight: "rgba(205, 85, 115, 0.07)",
    weakDark: "rgba(245, 120, 155, 0.10)",
    activeLight: "rgba(195, 75, 105, 0.25)",
    activeDark: "rgba(245, 120, 155, 0.30)",
  },
  {
    weakLight: "rgba(50, 160, 165, 0.07)",
    weakDark: "rgba(80, 195, 200, 0.10)",
    activeLight: "rgba(40, 150, 155, 0.25)",
    activeDark: "rgba(80, 195, 200, 0.30)",
  },
]

export class ActonAssemblyPreviewProvider implements vscode.TextDocumentContentProvider {
  public static readonly scheme: string = "acton-assembly-preview"

  private readonly onDidChangeEmitter: vscode.EventEmitter<vscode.Uri> =
    new vscode.EventEmitter<vscode.Uri>()
  public readonly onDidChange: vscode.Event<vscode.Uri> = this.onDidChangeEmitter.event

  private readonly contentByPreviewUri: Map<string, string> = new Map()
  private readonly stateByPreviewUri: Map<string, AssemblyPreviewState> = new Map()
  private readonly previewUriBySourceUri: Map<string, string> = new Map()
  private readonly paletteDecorations: readonly vscode.TextEditorDecorationType[] =
    BLOCK_PALETTE.map(colors =>
      vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        light: {backgroundColor: colors.weakLight},
        dark: {backgroundColor: colors.weakDark},
      }),
    )
  private readonly activeDecorations: readonly vscode.TextEditorDecorationType[] =
    BLOCK_PALETTE.map(colors =>
      vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        light: {backgroundColor: colors.activeLight},
        dark: {backgroundColor: colors.activeDark},
        overviewRulerLane: vscode.OverviewRulerLane.Center,
      }),
    )

  public register(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
      this.onDidChangeEmitter,
      ...this.paletteDecorations,
      ...this.activeDecorations,
      vscode.workspace.registerTextDocumentContentProvider(
        ActonAssemblyPreviewProvider.scheme,
        this,
      ),
      vscode.commands.registerCommand(
        "ton.acton.openAssemblyPreview",
        async (fileUri?: vscode.Uri) => {
          await this.open(fileUri)
        },
      ),
      vscode.commands.registerCommand(
        "ton.acton.refreshAssemblyPreview",
        async (resourceUri?: vscode.Uri) => {
          await this.refresh(resourceUri)
        },
      ),
      vscode.window.onDidChangeTextEditorSelection(event => {
        this.updateDecorations(event.textEditor)
      }),
      vscode.window.onDidChangeVisibleTextEditors(() => {
        this.applyPaletteDecorations()
        this.updateDecorations(vscode.window.activeTextEditor)
      }),
      vscode.window.tabGroups.onDidChangeTabs(event => {
        for (const tab of event.closed) {
          const uri = this.uriFromTab(tab)
          if (uri?.scheme === ActonAssemblyPreviewProvider.scheme) {
            this.forgetPreview(uri)
          }
        }
      }),
      vscode.workspace.onDidChangeTextDocument(event => {
        if (event.document.uri.scheme === ActonAssemblyPreviewProvider.scheme) {
          this.applyPaletteDecorations()
          this.updateDecorations(vscode.window.activeTextEditor)
        }
      }),
      vscode.workspace.onDidCloseTextDocument(document => {
        if (document.uri.scheme === ActonAssemblyPreviewProvider.scheme) {
          this.forgetPreview(document.uri)
        }
      }),
    )
  }

  public provideTextDocumentContent(uri: vscode.Uri): string {
    return this.contentByPreviewUri.get(uri.toString()) ?? this.formatLoadingText(uri)
  }

  public async open(fileUri?: vscode.Uri): Promise<void> {
    const sourceUri = this.resolveSourceUri(fileUri)
    if (!sourceUri) {
      await vscode.window.showErrorMessage("Open a Tolk file to disassemble it.")
      return
    }

    if (!sourceUri.fsPath.endsWith(".tolk")) {
      await vscode.window.showErrorMessage("Assembly preview is available only for Tolk files.")
      return
    }

    const previewUri = this.createPreviewUri(sourceUri)
    this.previewUriBySourceUri.set(sourceUri.toString(), previewUri.toString())
    this.contentByPreviewUri.set(previewUri.toString(), this.formatLoadingText(previewUri))
    this.onDidChangeEmitter.fire(previewUri)

    const document = await vscode.workspace.openTextDocument(previewUri)
    const tasmDocument = await vscode.languages.setTextDocumentLanguage(document, "tasm")
    await vscode.window.showTextDocument(tasmDocument, {
      preview: false,
      viewColumn: vscode.ViewColumn.Beside,
    })

    await this.render(previewUri, sourceUri)
  }

  private async refresh(resourceUri?: vscode.Uri): Promise<void> {
    const editorUri = resourceUri ?? vscode.window.activeTextEditor?.document.uri
    if (editorUri?.scheme === ActonAssemblyPreviewProvider.scheme) {
      await this.render(editorUri, this.sourceUriFromPreviewUri(editorUri))
      return
    }

    await this.open(editorUri)
  }

  private async render(previewUri: vscode.Uri, sourceUri: vscode.Uri): Promise<void> {
    const previewKey = previewUri.toString()
    this.contentByPreviewUri.set(previewKey, this.formatLoadingText(previewUri))
    this.stateByPreviewUri.delete(previewKey)
    this.onDidChangeEmitter.fire(previewUri)

    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Window,
        title: `Acton: disassembling ${path.basename(sourceUri.fsPath)}`,
      },
      async () => {
        try {
          const output = await this.compileAndDisassemble(sourceUri)
          if (this.isTabOpen(previewUri)) {
            this.contentByPreviewUri.set(previewKey, output.assemblyText)
            this.stateByPreviewUri.set(previewKey, {
              sourceUri,
              previewUri,
              blocks: output.blocks,
            })
          }
        } catch (error) {
          if (this.isTabOpen(previewUri)) {
            const message = error instanceof Error ? error.message : String(error)
            this.contentByPreviewUri.set(previewKey, this.formatFailureText(message))
          }
        } finally {
          if (this.isTabOpen(previewUri)) {
            this.onDidChangeEmitter.fire(previewUri)
            this.applyPaletteDecorations()
            this.updateDecorations(vscode.window.activeTextEditor)
          } else {
            this.forgetPreview(previewUri)
          }
        }
      },
    )
  }

  private async compileAndDisassemble(
    sourceUri: vscode.Uri,
  ): Promise<{readonly assemblyText: string; readonly blocks: readonly AssemblyPreviewBlock[]}> {
    await this.saveSourceDocument(sourceUri)

    const tomlUri = await Acton.getInstance().findActonToml(sourceUri)
    const workingDirectory = tomlUri ? path.dirname(tomlUri.fsPath) : path.dirname(sourceUri.fsPath)
    const tempDirectory = await fs.mkdtemp(path.join(os.tmpdir(), "acton-assembly-preview-"))
    const sourceMapPath = path.join(tempDirectory, "source-map.json")

    try {
      const compileCommand = new CompileCommand(sourceUri.fsPath)
      compileCommand.json = true
      compileCommand.sourceMapFile = sourceMapPath
      compileCommand.allowNoEntrypoint = true

      const compileResult = await Acton.getInstance().spawn(compileCommand, workingDirectory)
      const compileOutput = this.commandOutputOrThrow(
        compileResult,
        `Failed to compile ${sourceUri.fsPath}`,
      )
      const compiledCode = this.parseCompileResult(compileOutput)

      const disasmCommand = new DisasmCommand("", compiledCode)
      disasmCommand.json = true
      disasmCommand.sourceMapFile = sourceMapPath

      const disasmResult = await Acton.getInstance().spawn(disasmCommand, workingDirectory)
      const disasmOutput = this.commandOutputOrThrow(
        disasmResult,
        "Failed to disassemble compiled code",
      )
      return this.parseDisasmResult(disasmOutput, sourceUri, workingDirectory)
    } finally {
      await fs.rm(tempDirectory, {recursive: true, force: true})
    }
  }

  private async saveSourceDocument(sourceUri: vscode.Uri): Promise<void> {
    const document = vscode.workspace.textDocuments.find(
      candidate => candidate.uri.toString() === sourceUri.toString(),
    )
    if (document?.isDirty) {
      await document.save()
    }
  }

  private commandOutputOrThrow(result: ActonSpawnResult, failureMessage: string): string {
    if (result.exitCode === 0) {
      return result.stdout
    }

    const stdout = stripAnsiCodes(result.stdout.trim())
    const stderr = stripAnsiCodes(result.stderr.trim())
    const jsonError = this.parseJsonError(stdout)
    const details = [stderr, jsonError ?? stdout].filter(part => part.trim() !== "").join("\n\n")
    throw new Error(details ? `${failureMessage}: ${details}` : failureMessage)
  }

  private parseCompileResult(output: string): string {
    const result = this.parseJson(output, "compile JSON") as TolkCompileJsonResult
    if (!result.success) {
      throw new Error(result.error ?? "Compilation failed")
    }
    if (!result.code_boc64 || result.code_boc64.trim() === "") {
      throw new Error("Compilation JSON did not include code_boc64")
    }
    return result.code_boc64
  }

  private parseDisasmResult(
    output: string,
    sourceUri: vscode.Uri,
    workingDirectory: string,
  ): {readonly assemblyText: string; readonly blocks: readonly AssemblyPreviewBlock[]} {
    const result = this.parseJson(output, "disasm JSON") as TolkDisasmJsonResult
    if (!result.success) {
      throw new Error(result.error ?? "Disassembly failed")
    }
    if (!result.assembly) {
      throw new Error("Disassembly JSON did not include assembly text")
    }

    return {
      assemblyText: result.assembly,
      blocks: this.buildPreviewBlocks(result.blocks ?? [], sourceUri, workingDirectory),
    }
  }

  private buildPreviewBlocks(
    jsonBlocks: readonly TolkDisasmJsonBlock[],
    sourceUri: vscode.Uri,
    workingDirectory: string,
  ): readonly AssemblyPreviewBlock[] {
    const sourcePath = this.normalizePath(sourceUri.fsPath, workingDirectory)
    const assemblyRangesBySourceLine: Map<number, AssemblyLineRange[]> = new Map()

    for (const block of jsonBlocks) {
      const source = block.source
      if (!source || this.normalizePath(source.file, workingDirectory) !== sourcePath) {
        continue
      }

      const assemblyRanges = this.mergeLineRanges(
        (block.assembly_ranges ?? []).flatMap(range => {
          if (range.start_line < 0 || range.end_line < range.start_line) {
            return []
          }
          return [{startLine: range.start_line, endLine: range.end_line}]
        }),
      )
      if (assemblyRanges.length === 0 || source.line <= 0) {
        continue
      }

      const sourceLine = source.line - 1
      const sourceRanges = assemblyRangesBySourceLine.get(sourceLine) ?? []
      sourceRanges.push(...assemblyRanges)
      assemblyRangesBySourceLine.set(sourceLine, sourceRanges)
    }

    return [...assemblyRangesBySourceLine.entries()]
      .sort(([left], [right]) => left - right)
      .map(([sourceLine, assemblyRanges], index) => ({
        sourceLine,
        paletteIndex: index % this.paletteDecorations.length,
        assemblyRanges: this.mergeLineRanges(assemblyRanges),
      }))
  }

  private mergeLineRanges(ranges: readonly AssemblyLineRange[]): readonly AssemblyLineRange[] {
    const sortedRanges = [...ranges].sort(
      (left, right) => left.startLine - right.startLine || left.endLine - right.endLine,
    )
    const merged: AssemblyLineRange[] = []
    let previous: AssemblyLineRange | undefined

    for (const range of sortedRanges) {
      if (!previous) {
        merged.push(range)
        previous = range
        continue
      }

      if (range.startLine > previous.endLine + 1) {
        merged.push(range)
        previous = range
        continue
      }

      const mergedRange = {
        startLine: previous.startLine,
        endLine: Math.max(previous.endLine, range.endLine),
      }
      merged[merged.length - 1] = mergedRange
      previous = mergedRange
    }

    return merged
  }

  private updateDecorations(editor: vscode.TextEditor | undefined): void {
    if (!editor) {
      return
    }

    const state = this.findPreviewState(editor.document.uri)
    if (!state) {
      this.clearActiveDecorations()
      return
    }

    const activeLine = editor.selection.active.line
    const isPreviewEditor = editor.document.uri.scheme === ActonAssemblyPreviewProvider.scheme
    const block = isPreviewEditor
      ? this.findBlockByAssemblyLine(state.blocks, activeLine)
      : this.findBlockBySourceLine(state.blocks, activeLine)
    if (!block) {
      this.clearActiveDecorations()
      return
    }

    this.applySelectionDecorations(state, block)
  }

  private findPreviewState(uri: vscode.Uri): AssemblyPreviewState | undefined {
    if (uri.scheme === ActonAssemblyPreviewProvider.scheme) {
      return this.stateByPreviewUri.get(uri.toString())
    }

    const previewUri = this.previewUriBySourceUri.get(uri.toString())
    return previewUri ? this.stateByPreviewUri.get(previewUri) : undefined
  }

  private findBlockBySourceLine(
    blocks: readonly AssemblyPreviewBlock[],
    line: number,
  ): AssemblyPreviewBlock | undefined {
    return blocks.find(candidate => candidate.sourceLine === line)
  }

  private findBlockByAssemblyLine(
    blocks: readonly AssemblyPreviewBlock[],
    line: number,
  ): AssemblyPreviewBlock | undefined {
    return blocks.find(block =>
      block.assemblyRanges.some(range => line >= range.startLine && line <= range.endLine),
    )
  }

  private applySelectionDecorations(
    state: AssemblyPreviewState,
    block: AssemblyPreviewBlock,
  ): void {
    this.clearActiveDecorations()
    const activeDecoration = this.activeDecorations[block.paletteIndex]

    const sourceEditor = this.findVisibleEditor(state.sourceUri)
    if (sourceEditor) {
      const sourceRange = this.createLineRange(sourceEditor.document, block.sourceLine)
      sourceEditor.setDecorations(activeDecoration, [sourceRange])
      sourceEditor.revealRange(sourceRange, vscode.TextEditorRevealType.InCenterIfOutsideViewport)
    }

    const previewEditor = this.findVisibleEditor(state.previewUri)
    if (previewEditor) {
      const assemblyRanges = block.assemblyRanges.map(range =>
        this.createLineRange(previewEditor.document, range.startLine, range.endLine),
      )
      previewEditor.setDecorations(activeDecoration, assemblyRanges)
      if (assemblyRanges[0]) {
        previewEditor.revealRange(
          assemblyRanges[0],
          vscode.TextEditorRevealType.InCenterIfOutsideViewport,
        )
      }
    }
  }

  private applyPaletteDecorations(): void {
    this.clearPaletteDecorations()

    for (const state of this.stateByPreviewUri.values()) {
      const sourceEditor = this.findVisibleEditor(state.sourceUri)
      if (sourceEditor) {
        this.applyPaletteDecorationsToEditor(
          sourceEditor,
          state.blocks.map(block => ({
            paletteIndex: block.paletteIndex,
            ranges: [this.createLineRange(sourceEditor.document, block.sourceLine)],
          })),
        )
      }

      const previewEditor = this.findVisibleEditor(state.previewUri)
      if (previewEditor) {
        this.applyPaletteDecorationsToEditor(
          previewEditor,
          state.blocks.flatMap(block =>
            block.assemblyRanges.map(range => ({
              paletteIndex: block.paletteIndex,
              ranges: [
                this.createLineRange(previewEditor.document, range.startLine, range.endLine),
              ],
            })),
          ),
        )
      }
    }
  }

  private forgetPreview(previewUri: vscode.Uri): void {
    const previewKey = previewUri.toString()
    const state = this.stateByPreviewUri.get(previewKey)
    const sourceUri = state?.sourceUri ?? this.sourceUriFromPreviewUri(previewUri)
    const mappedPreview = this.previewUriBySourceUri.get(sourceUri.toString())
    if (mappedPreview === previewKey) {
      this.previewUriBySourceUri.delete(sourceUri.toString())
    }

    this.stateByPreviewUri.delete(previewKey)
    this.contentByPreviewUri.delete(previewKey)
    this.clearPaletteDecorations()
    this.clearActiveDecorations()
    this.applyPaletteDecorations()
  }

  private applyPaletteDecorationsToEditor(
    editor: vscode.TextEditor,
    groups: readonly {
      readonly paletteIndex: number
      readonly ranges: readonly vscode.Range[]
    }[],
  ): void {
    for (let i = 0; i < this.paletteDecorations.length; i++) {
      const ranges = groups
        .filter(group => group.paletteIndex === i)
        .flatMap(group => [...group.ranges])
      editor.setDecorations(this.paletteDecorations[i], ranges)
    }
  }

  private clearPaletteDecorations(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      for (const decoration of this.paletteDecorations) {
        editor.setDecorations(decoration, [])
      }
    }
  }

  private clearActiveDecorations(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      for (const decoration of this.activeDecorations) {
        editor.setDecorations(decoration, [])
      }
    }
  }

  private findVisibleEditor(uri: vscode.Uri): vscode.TextEditor | undefined {
    return vscode.window.visibleTextEditors.find(
      editor => editor.document.uri.toString() === uri.toString(),
    )
  }

  private uriFromTab(tab: vscode.Tab): vscode.Uri | undefined {
    if (tab.input instanceof vscode.TabInputText) {
      return tab.input.uri
    }
    return undefined
  }

  private isTabOpen(uri: vscode.Uri): boolean {
    const key = uri.toString()
    return vscode.window.tabGroups.all.some(group =>
      group.tabs.some(tab => this.uriFromTab(tab)?.toString() === key),
    )
  }

  private createLineRange(
    document: vscode.TextDocument,
    startLine: number,
    endLine: number = startLine,
  ): vscode.Range {
    const lastLine = Math.max(0, document.lineCount - 1)
    const start = Math.max(0, Math.min(startLine, lastLine))
    const end = Math.max(start, Math.min(endLine, lastLine))
    return new vscode.Range(start, 0, end, document.lineAt(end).text.length)
  }

  private resolveSourceUri(fileUri: vscode.Uri | undefined): vscode.Uri | undefined {
    if (fileUri?.scheme === ActonAssemblyPreviewProvider.scheme) {
      return this.sourceUriFromPreviewUri(fileUri)
    }
    return fileUri ?? vscode.window.activeTextEditor?.document.uri
  }

  private createPreviewUri(sourceUri: vscode.Uri): vscode.Uri {
    const query = new URLSearchParams({source: sourceUri.toString()}).toString()
    return sourceUri.with({
      scheme: ActonAssemblyPreviewProvider.scheme,
      path: `${sourceUri.path}${PREVIEW_SUFFIX}`,
      query,
      fragment: "",
    })
  }

  private sourceUriFromPreviewUri(previewUri: vscode.Uri): vscode.Uri {
    const source = new URLSearchParams(previewUri.query).get("source")
    if (source) {
      return vscode.Uri.parse(source)
    }

    return previewUri.with({
      scheme: "file",
      path: this.stripPreviewSuffix(previewUri.path),
      query: "",
      fragment: "",
    })
  }

  private formatLoadingText(uri: vscode.Uri): string {
    const sourceUri =
      uri.scheme === ActonAssemblyPreviewProvider.scheme ? this.sourceUriFromPreviewUri(uri) : uri
    return [
      `// Assembly preview for ${sourceUri.fsPath}`,
      "// Compiling and disassembling with Acton...",
      "",
    ].join("\n")
  }

  private formatFailureText(message: string): string {
    return stripAnsiCodes(message)
      .split(/\r?\n/)
      .map(line => `// ${line}`)
      .join("\n")
  }

  private parseJson(output: string, label: string): unknown {
    const text = stripAnsiCodes(output).trim()
    try {
      return JSON.parse(text) as unknown
    } catch (error) {
      const jsonStart = text.indexOf("{")
      if (jsonStart > 0) {
        try {
          return JSON.parse(text.slice(jsonStart)) as unknown
        } catch {
          // Keep the original parse error below; it points at the real command output.
        }
      }
      const message = error instanceof Error ? error.message : String(error)
      throw new Error(`Failed to parse ${label}: ${message}`)
    }
  }

  private parseJsonError(output: string): string | undefined {
    if (output.trim() === "") {
      return undefined
    }

    try {
      const parsed = this.parseJson(output, "error JSON") as {readonly error?: string}
      return parsed.error
    } catch {
      return undefined
    }
  }

  private normalizePath(filePath: string, workingDirectory: string): string {
    const absolutePath = path.isAbsolute(filePath)
      ? filePath
      : path.resolve(workingDirectory, filePath)
    return path.normalize(absolutePath)
  }

  private stripPreviewSuffix(uriPath: string): string {
    return uriPath.endsWith(PREVIEW_SUFFIX) ? uriPath.slice(0, -PREVIEW_SUFFIX.length) : uriPath
  }
}
