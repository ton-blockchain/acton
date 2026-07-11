//  SPDX-License-Identifier: MIT
//  Copyright © 2025 TON Studio
import * as vscode from "vscode"

import {BocDecompilerProvider} from "./BocDecompilerProvider"

export class BocFileSystemProvider implements vscode.FileSystemProvider {
  private readonly _emitter: vscode.EventEmitter<vscode.FileChangeEvent[]> =
    new vscode.EventEmitter()
  public readonly onDidChangeFile: vscode.Event<vscode.FileChangeEvent[]> = this._emitter.event

  public watch(_uri: vscode.Uri): vscode.Disposable {
    return new vscode.Disposable(() => {})
  }

  public stat(_uri: vscode.Uri): vscode.FileStat {
    return {
      type: vscode.FileType.File,
      ctime: Date.now(),
      mtime: Date.now(),
      size: 0,
    }
  }

  public readDirectory(_uri: vscode.Uri): [string, vscode.FileType][] {
    return []
  }

  public createDirectory(_uri: vscode.Uri): void {}

  public async readFile(uri: vscode.Uri): Promise<Uint8Array> {
    console.log("Reading BOC file:", uri.fsPath)
    try {
      const fileContent = await vscode.workspace.fs.readFile(uri)
      console.log("File content length:", fileContent.length)

      const decompileUri = uri.with({
        scheme: BocDecompilerProvider.scheme,
        path: uri.path + ".decompiled.tasm",
      })
      console.log("Disassemble URI:", decompileUri.toString())

      const doc = await vscode.workspace.openTextDocument(decompileUri)
      await vscode.window.showTextDocument(doc, {
        preview: true,
        viewColumn: vscode.ViewColumn.Active,
      })

      return fileContent
    } catch (error) {
      console.error("Error reading BOC file:", error)
      throw vscode.FileSystemError.FileNotFound(uri)
    }
  }

  public writeFile(_uri: vscode.Uri, _content: Uint8Array): void {}

  public delete(_uri: vscode.Uri): void {}

  public rename(_oldUri: vscode.Uri, _newUri: vscode.Uri): void {}
}
