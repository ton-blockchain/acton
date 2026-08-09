import type {EnvironmentConfig, EnvironmentStatus, StudioEnvironment} from "./studioApi"

export const environmentStatusLabels = {
  starting: "Starting",
  running: "Running",
  stopping: "Stopping",
  stopped: "Stopped",
  failed: "Failed",
} satisfies Record<EnvironmentStatus, string>

export function formatEnvironmentType(config: EnvironmentConfig) {
  if (config.kind === "actonLocalnet") return "Simulated localnet"
  if (config.kind === "fullTonNetwork") return "Full localnet"
  return config.network === "mainnet" ? "Mainnet" : "Testnet"
}

export function formatEnvironmentNetwork(environment: StudioEnvironment) {
  if (
    environment.config.kind === "fullTonNetwork" ||
    (environment.config.kind === "actonLocalnet" && !environment.config.forkNetwork)
  ) {
    return "Clean localnet"
  }

  return environment.network.label
}
