const LEGACY_STORAGE_KEYS = [
  "addressHistory",
  "i18nextLng",
  "ipfs:",
  "loglevel",
  "ton-connect-storage_bridge-connection",
  "ton-connect-storage_http-bridge-gateway::",
  "ton-connect-ui_preferred-wallet",
  "ton-connect-ui_wallet-info",
] as const

export function clearLegacyVerifierStorage(): void {
  try {
    const storage = globalThis.localStorage
    const keys = Array.from({length: storage.length}, (_, index) => storage.key(index))

    for (const key of keys) {
      if (key && isLegacyStorageKey(key)) {
        storage.removeItem(key)
      }
    }
  } catch (error) {
    console.warn("Failed to clear legacy verifier storage", error)
  }
}

function isLegacyStorageKey(key: string): boolean {
  return LEGACY_STORAGE_KEYS.some(legacyKey => key.startsWith(legacyKey))
}
