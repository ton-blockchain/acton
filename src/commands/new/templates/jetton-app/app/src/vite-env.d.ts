/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly TONCENTER_MAINNET_API_KEY?: string;
  readonly TONCENTER_TESTNET_API_KEY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
