import {
  createEmulationShareResponse,
  type EmulationSharePagesContext,
} from "../../../worker/emulationShares"

export function onRequest(context: EmulationSharePagesContext): Promise<Response> {
  return createEmulationShareResponse(context)
}
