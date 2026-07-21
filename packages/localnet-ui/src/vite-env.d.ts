/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_LOCALNET_HOST?: string
  readonly VITE_LOCALNET_API_TOKEN?: string
  readonly VITE_LOCALNET_TONCENTER_API_KEY?: string
  readonly VITE_LOCALNET_TONCENTER_API_V2_URL?: string
  readonly VITE_LOCALNET_TONCENTER_API_V3_URL?: string
}

declare module "*.module.css" {
  const classes: {[key: string]: string}
  export default classes
}
