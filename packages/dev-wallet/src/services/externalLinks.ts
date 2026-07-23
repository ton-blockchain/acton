import {openUrl} from "@tauri-apps/plugin-opener"

import {isTauriRuntime} from "./walletVault"

export async function openExternalUrl(url: string): Promise<void> {
  if (isTauriRuntime()) {
    await openUrl(url)
    return
  }

  const opened = globalThis.open(url, "_blank", "noopener,noreferrer")
  if (!opened) {
    globalThis.location.assign(url)
  }
}
