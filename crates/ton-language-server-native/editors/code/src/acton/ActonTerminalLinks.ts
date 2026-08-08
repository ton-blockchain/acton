//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core

import * as vscode from "vscode"

import {Acton} from "./Acton"
import {
  createTonAddressExplorerUrl,
  findTonAddressMatches,
  normalizeTonExplorer,
  type TonAddressMatch,
} from "./ActonTonAddress"

const BACKTRACE_HINT_PATTERN = /re-run with\s+(--backtrace\s+full)/i

class TonAddressTerminalLink extends vscode.TerminalLink {
  public constructor(private readonly match: TonAddressMatch) {
    super(match.startIndex, match.length, "Open TON address in explorer")
  }

  public get address(): string {
    return this.match.address
  }

  public get isTestnet(): boolean {
    return this.match.isTestnet
  }
}

class ActonTonAddressTerminalLinkProvider
  implements vscode.TerminalLinkProvider<TonAddressTerminalLink>
{
  public provideTerminalLinks(
    context: vscode.TerminalLinkContext,
    _token: vscode.CancellationToken,
  ): TonAddressTerminalLink[] {
    return findTonAddressMatches(context.line).map(match => new TonAddressTerminalLink(match))
  }

  public async handleTerminalLink(link: TonAddressTerminalLink): Promise<void> {
    const explorer = normalizeTonExplorer(
      vscode.workspace.getConfiguration("ton").get<string>("acton.explorer"),
    )
    const url = createTonAddressExplorerUrl(link.address, explorer, link.isTestnet)
    await vscode.env.openExternal(vscode.Uri.parse(url))
  }
}

class ActonBacktraceTerminalLinkProvider implements vscode.TerminalLinkProvider {
  public provideTerminalLinks(
    context: vscode.TerminalLinkContext,
    _token: vscode.CancellationToken,
  ): vscode.TerminalLink[] {
    const match = BACKTRACE_HINT_PATTERN.exec(context.line)
    if (!match) {
      return []
    }

    if (!Acton.getInstance().hasScriptFullBacktraceCommand()) {
      return []
    }

    const linkText = match[1]
    const linkStart = match.index + match[0].indexOf(linkText)
    return [
      new vscode.TerminalLink(
        linkStart,
        linkText.length,
        "Re-run last Acton script with --backtrace full",
      ),
    ]
  }

  public async handleTerminalLink(_link: vscode.TerminalLink): Promise<void> {
    const didRun = await Acton.getInstance().rerunLastScriptWithFullBacktrace()
    if (!didRun) {
      await vscode.window.showWarningMessage(
        "No Acton script command is available to re-run with full backtrace.",
      )
    }
  }
}

export function registerActonTerminalLinks(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.window.registerTerminalLinkProvider(new ActonTonAddressTerminalLinkProvider()),
    vscode.window.registerTerminalLinkProvider(new ActonBacktraceTerminalLinkProvider()),
  )
}
