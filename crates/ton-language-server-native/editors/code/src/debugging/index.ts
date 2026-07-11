import * as vscode from "vscode"
import type {DebugAdapterExecutable, DebugSession} from "vscode"

export function configureDebugging(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider("acton", {
      resolveDebugConfiguration(
        _folder: vscode.WorkspaceFolder | undefined,
        config: vscode.DebugConfiguration,
      ) {
        const port = Number(config.port)
        if (!Number.isInteger(port) || port <= 0) {
          void vscode.window.showErrorMessage(
            "Acton debug configuration requires a valid TCP port.",
          )
          return undefined
        }

        return {
          ...config,
          host:
            typeof config.host === "string" && config.host.trim() !== ""
              ? config.host
              : "127.0.0.1",
          port,
          request: config.request.trim() === "" ? "launch" : config.request,
        }
      },
    }),
    vscode.debug.registerDebugAdapterDescriptorFactory("acton", {
      createDebugAdapterDescriptor(session: DebugSession, _e: DebugAdapterExecutable | undefined) {
        const port = Number(session.configuration.port)
        const host = String(session.configuration.host ?? "127.0.0.1")
        return new vscode.DebugAdapterServer(port, host)
      },
    }),
  )
}
