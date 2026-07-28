import type {EnvironmentCapability, StudioEnvironment} from "./studioApi"

export function supports(
  environment: StudioEnvironment | undefined,
  capability: EnvironmentCapability,
): boolean {
  return environment?.capabilities.includes(capability) ?? false
}
