/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_TON_NETWORK?: 'mainnet' | 'testnet';
  readonly VITE_TONCENTER_API_KEY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
