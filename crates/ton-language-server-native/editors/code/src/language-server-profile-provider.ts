import * as vscode from "vscode"

import {sendLanguageServerRequest} from "./language-server"
import {type LanguageServerProfile, renderLanguageServerProfile} from "./language-server-profile"

const refreshIntervalMs = 500

export class LanguageServerProfileProvider
  implements vscode.TextDocumentContentProvider, vscode.Disposable
{
  public static readonly scheme = "acton-language-server-profile"

  private readonly uri = vscode.Uri.from({
    scheme: LanguageServerProfileProvider.scheme,
    path: "/profile.txt",
  })
  private readonly onDidChangeEmitter = new vscode.EventEmitter<vscode.Uri>()
  public readonly onDidChange = this.onDidChangeEmitter.event
  private refreshTimer: ReturnType<typeof setInterval> | undefined

  public register(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
      this,
      vscode.workspace.registerTextDocumentContentProvider(
        LanguageServerProfileProvider.scheme,
        this,
      ),
      vscode.commands.registerCommand("ton.showLanguageServerProfile", () => this.open()),
      vscode.workspace.onDidOpenTextDocument(document => {
        if (document.uri.toString() === this.uri.toString()) {
          this.startRefreshing()
        }
      }),
      vscode.workspace.onDidCloseTextDocument(document => {
        if (document.uri.toString() === this.uri.toString()) {
          this.stopRefreshing()
        }
      }),
    )
  }

  public async provideTextDocumentContent(): Promise<string> {
    try {
      const profile = await sendLanguageServerRequest<LanguageServerProfile>("ton/profile", {})
      return renderLanguageServerProfile(profile)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      return `Acton Language Server Profile\n\nFailed to load profile: ${message}\n`
    }
  }

  public dispose(): void {
    this.stopRefreshing()
    this.onDidChangeEmitter.dispose()
  }

  private async open(): Promise<void> {
    const document = await vscode.workspace.openTextDocument(this.uri)
    await vscode.window.showTextDocument(document, {
      preview: false,
      viewColumn: vscode.ViewColumn.Beside,
    })
    this.startRefreshing()
  }

  private startRefreshing(): void {
    if (this.refreshTimer !== undefined) {
      return
    }
    this.onDidChangeEmitter.fire(this.uri)
    this.refreshTimer = setInterval(() => {
      this.onDidChangeEmitter.fire(this.uri)
    }, refreshIntervalMs)
  }

  private stopRefreshing(): void {
    if (this.refreshTimer === undefined) {
      return
    }
    clearInterval(this.refreshTimer)
    this.refreshTimer = undefined
  }
}
