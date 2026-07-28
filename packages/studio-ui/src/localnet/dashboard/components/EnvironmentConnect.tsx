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

  return (
    <EnvironmentConnectPanel
      apiV2Url={environment?.endpoints.apiV2}
      apiV3Url={environment?.endpoints.apiV3}
      controlUrl={environment?.endpoints.control}
      environmentName={environment?.name ?? "Virtual environment"}
      explorerUrl={supports(environment, "explorer") ? routes.path("/explorer") : undefined}
      integratePath={routes.path("/integrate")}
      onDismiss={onDismiss}
      settingsPath={routes.path("/settings")}
    />
  )
}
