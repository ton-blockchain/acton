/// <reference types="vite/client" />

// eslint-disable-next-line functional/type-declaration-immutability
interface ImportMetaEnv {
  readonly VITE_LOCALNET_HOST?: string
  readonly VITE_LOCALNET_TONCENTER_API_KEY?: string
}

declare module "*.module.css" {
  const classes: {[key: string]: string}
  export default classes
}
