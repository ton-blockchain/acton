import {
  readEmulationShareResponse,
  type EmulationSharePagesContext,
} from "../../../worker/emulationShares"

export function onRequest(context: EmulationSharePagesContext): Promise<Response> {
  return readEmulationShareResponse(context)
}
