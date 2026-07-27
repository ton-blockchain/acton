import {useMemo} from "react"
import type {FC} from "react"

import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import type {TonClient} from "../../explorer/api/client"
import {useLocalnetRoutes} from "../../routes"

import {EnvironmentConnectPanel} from "./EnvironmentConnectPanel"

interface EnvironmentConnectProps {
  readonly client: TonClient
  readonly onDismiss?: () => void
}

export const EnvironmentConnect: FC<EnvironmentConnectProps> = ({client, onDismiss}) => {
  const runtime = useLocalnetRuntime()
  const routes = useLocalnetRoutes()
  const endpoints = useMemo(() => client.getEndpoints(), [client])

  return (
    <EnvironmentConnectPanel
      apiV2Url={endpoints.apiV2}
      apiV3Url={endpoints.apiV3}
      controlUrl={endpoints.admin}
      environmentName={runtime.environment?.name ?? "Virtual environment"}
      explorerUrl={routes.path("/explorer")}
      integratePath={routes.path("/integrate")}
      onDismiss={onDismiss}
      rpcUrl={runtime.environment?.rpcUrl ?? endpoints.admin}
      settingsPath={routes.path("/settings")}
    />
  )
}
