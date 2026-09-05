import {createOpenAPI} from "fumadocs-openapi/server"

export const openapi = createOpenAPI({
  input: ["./public/openapi/acton-simulated-localnet-control.openapi.json"],
})
