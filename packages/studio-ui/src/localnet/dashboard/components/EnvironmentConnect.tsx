import type {FC} from "react"

import {supports} from "../../../environmentCapabilities"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import {useLocalnetRoutes} from "../../routes"

import {EnvironmentConnectPanel} from "./EnvironmentConnectPanel"

interface EnvironmentConnectProps {
  readonly onDismiss?: () => void
}

export const EnvironmentConnect: FC<EnvironmentConnectProps> = ({onDismiss}) => {
  const runtime = useLocalnetRuntime()
  const routes = useLocalnetRoutes()
  const environment = runtime.environment
  const remoteNetwork =
    environment?.config.kind === "remoteTonNetwork" ? environment.config.network : undefined

  return (
    <EnvironmentConnectPanel
      actonNetworkName={remoteNetwork ?? "localnet"}
      apiV2Url={environment?.endpoints.apiV2}
      apiV3Url={environment?.endpoints.apiV3}
      configureActonNetwork={remoteNetwork === undefined}
      controlUrl={environment?.endpoints.control}
      environmentName={environment?.name ?? "Virtual environment"}
      explorerUrl={supports(environment, "explorer") ? routes.path("/explorer") : undefined}
      integratePath={routes.path("/integrate")}
      onDismiss={onDismiss}
      settingsPath={environment?.lifecycle === "managed" ? routes.path("/settings") : undefined}
    />
  )
}
