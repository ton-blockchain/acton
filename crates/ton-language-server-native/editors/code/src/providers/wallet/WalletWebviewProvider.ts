//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as vscode from "vscode"

import {Acton} from "../../acton/Acton"
import {
  WalletAirdropCommand,
  WalletImportCommand,
  WalletListCommand,
  WalletNewCommand,
} from "../../acton/ActonCommand"
import {createTonAddressExplorerUrl, normalizeTonExplorer} from "../../acton/ActonTonAddress"
import type {
  WebviewWalletCommand,
  WalletInfo,
  WalletListInfo,
  WalletMessage,
} from "../../webview-ui/src/views/wallet/wallet-types"

export class WalletWebviewProvider implements vscode.WebviewViewProvider {
  public static readonly viewType: string = "tonWallets"

  private view?: vscode.WebviewView
  private wallets: readonly WalletInfo[] = []
  private isLoading: boolean = false

  public constructor(private readonly _extensionUri: vscode.Uri) {}

  public registerCommands(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
      vscode.commands.registerCommand("ton.wallets.refresh", async () => {
        await this.handleLoadWallets()
      }),
      vscode.commands.registerCommand("ton.wallets.new", () => {
        this.view?.webview.postMessage({type: "showNewWalletForm"})
      }),
      vscode.commands.registerCommand("ton.wallets.import", () => {
        this.view?.webview.postMessage({type: "showImportWalletForm"})
      }),
    )
  }

  public resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken,
  ): void {
    this.view = webviewView

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [this._extensionUri],
    }

    webviewView.webview.html = this.getHtmlForWebview(webviewView.webview)

    webviewView.webview.onDidReceiveMessage(async (command: WebviewWalletCommand) => {
      switch (command.type) {
        case "loadWallets": {
          await this.handleLoadWallets()
          break
        }
        case "webviewReady": {
          await this.handleLoadWallets()
          break
        }
        case "newWallet": {
          await this.handleNewWallet(command)
          break
        }
        case "importWallet": {
          await this.handleImportWallet(command)
          break
        }
        case "airdrop": {
          await this.handleAirdrop(command.walletName)
          break
        }
        case "copyAddress": {
          await vscode.env.clipboard.writeText(command.address)
          void vscode.window.showInformationMessage("Address copied to clipboard")
          break
        }
        case "openInExplorer": {
          const explorer = normalizeTonExplorer(
            vscode.workspace.getConfiguration("ton").get<string>("acton.explorer"),
          )
          const url = createTonAddressExplorerUrl(command.address, explorer, true)
          void vscode.env.openExternal(vscode.Uri.parse(url))
          break
        }
      }
    })
  }

  private async handleLoadWallets(): Promise<void> {
    this.isLoading = true
    this.updateWebview()

    try {
      const acton = Acton.getInstance()
      const workspaceFolder = vscode.workspace.workspaceFolders?.[0]
      if (!workspaceFolder) {
        this.wallets = []
        return
      }

      const command = new WalletListCommand(true)
      const result = await acton.spawn(command, workspaceFolder.uri.fsPath)

      if (result.exitCode === 0) {
        const info = JSON.parse(result.stdout) as WalletListInfo
        if (info.success) {
          this.wallets = info.wallets
        } else {
          void vscode.window.showErrorMessage("Failed to list wallets")
        }
      } else {
        void vscode.window.showErrorMessage(
          `Error listing wallets: ${result.stderr || result.stdout}`,
        )
      }
    } catch (error) {
      console.error("Failed to load wallets:", error)
      void vscode.window.showErrorMessage("Failed to load wallets")
    } finally {
      this.isLoading = false
      this.updateWebview()
    }
  }

  private async handleNewWallet(command: {
    readonly name: string
    readonly version: string
    readonly global: boolean
    readonly secure: boolean
  }): Promise<void> {
    try {
      const acton = Acton.getInstance()
      const workspaceFolder = vscode.workspace.workspaceFolders?.[0]
      if (!workspaceFolder) return

      const actonCommand = new WalletNewCommand(
        command.name,
        command.version,
        command.global,
        command.secure,
      )
      const result = await acton.spawn(actonCommand, workspaceFolder.uri.fsPath)

      if (result.exitCode === 0) {
        void vscode.window.showInformationMessage(`Wallet "${command.name}" created successfully`)
        await this.handleLoadWallets()
      } else {
        void vscode.window.showErrorMessage(
          `Error creating wallet: ${result.stderr || result.stdout}`,
        )
      }
    } catch (error) {
      console.error("Failed to create wallet:", error)
      void vscode.window.showErrorMessage("Failed to create wallet")
    }
  }

  private async handleImportWallet(command: {
    readonly name: string
    readonly mnemonic: string
    readonly version: string
    readonly global: boolean
    readonly secure: boolean
  }): Promise<void> {
    try {
      const acton = Acton.getInstance()
      const workspaceFolder = vscode.workspace.workspaceFolders?.[0]
      if (!workspaceFolder) return

      const actonCommand = new WalletImportCommand(
        command.name,
        command.mnemonic,
        command.version,
        command.global,
        command.secure,
      )
      const result = await acton.spawn(actonCommand, workspaceFolder.uri.fsPath)

      if (result.exitCode === 0) {
        void vscode.window.showInformationMessage(`Wallet "${command.name}" imported successfully`)
        await this.handleLoadWallets()
      } else {
        void vscode.window.showErrorMessage(
          `Error importing wallet: ${result.stderr || result.stdout}`,
        )
      }
    } catch (error) {
      console.error("Failed to import wallet:", error)
      void vscode.window.showErrorMessage("Failed to import wallet")
    }
  }

  private async handleAirdrop(walletName: string): Promise<void> {
    this.view?.webview.postMessage({type: "airdropStatus", walletName, status: "requesting"})

    try {
      const acton = Acton.getInstance()
      const workspaceFolder = vscode.workspace.workspaceFolders?.[0]
      if (!workspaceFolder) return

      const command = new WalletAirdropCommand(walletName)
      const result = await acton.spawn(command, workspaceFolder.uri.fsPath)

      if (result.exitCode === 0) {
        const res = JSON.parse(result.stdout) as {
          readonly success: boolean
          readonly message?: string
          readonly error?: string
        }
        if (res.success) {
          void vscode.window.showInformationMessage(res.message ?? "Airdrop successful")
          await this.handleLoadWallets()
        } else {
          void vscode.window.showErrorMessage(res.error ?? res.message ?? "Airdrop failed")
        }
      } else {
        void vscode.window.showErrorMessage(`Airdrop error: ${result.stderr || result.stdout}`)
      }
    } catch (error) {
      console.error("Airdrop failed:", error)
      void vscode.window.showErrorMessage("Airdrop failed")
    } finally {
      this.view?.webview.postMessage({type: "airdropStatus", walletName, status: "idle"})
    }
  }

  private updateWebview(): void {
    if (this.view) {
      const message: WalletMessage = {
        type: "updateWallets",
        wallets: this.wallets,
        isLoading: this.isLoading,
      }
      void this.view.webview.postMessage(message)
    }
  }

  private getHtmlForWebview(webview: vscode.Webview): string {
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this._extensionUri, "dist", "webview-ui", "wallet.js"),
    )
    const styleUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this._extensionUri, "dist", "webview-ui", "wallet.css"),
    )

    const nonce = getNonce()

    return `<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <link href="${styleUri.toString()}" rel="stylesheet">
                <title>TON Wallets</title>
            </head>
            <body>
                <div id="root"></div>
                <script nonce="${nonce}" src="${scriptUri.toString()}"></script>
            </body>
            </html>`
  }
}

function getNonce(): string {
  let text = ""
  const possible = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length))
  }
  return text
}
