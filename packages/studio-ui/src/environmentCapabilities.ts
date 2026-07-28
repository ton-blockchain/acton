import type {EnvironmentCapability, StudioEnvironment} from "./studioApi"

export function supports(
  environment: StudioEnvironment | undefined,
  capability: EnvironmentCapability,
): boolean {
  return environment?.capabilities.includes(capability) ?? false
}

export function supportsAny(
  environment: StudioEnvironment | undefined,
  ...capabilities: readonly EnvironmentCapability[]
): boolean {
  return capabilities.some(capability => supports(environment, capability))
}
