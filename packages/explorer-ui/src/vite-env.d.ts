/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_EXPLORER_TONCENTER_API_KEY?: string
  readonly VITE_EXPLORER_TONCENTER_API_V2_URL?: string
  readonly VITE_EXPLORER_TONCENTER_API_V3_URL?: string
  readonly VITE_EXPLORER_MAINNET_TONCENTER_API_KEY?: string
  readonly VITE_EXPLORER_MAINNET_TONCENTER_API_V2_URL?: string
  readonly VITE_EXPLORER_MAINNET_TONCENTER_API_V3_URL?: string
  readonly VITE_EXPLORER_TESTNET_TONCENTER_API_KEY?: string
  readonly VITE_EXPLORER_TESTNET_TONCENTER_API_V2_URL?: string
  readonly VITE_EXPLORER_TESTNET_TONCENTER_API_V3_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
