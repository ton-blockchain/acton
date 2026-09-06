import {createObservabilityClient, SessionStats} from "@acton/localton-ui"
import {useMemo} from "react"
import type {FC} from "react"

import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"

/** Connects validator session charts to the selected Full Localnet observer */
export const StatsPage: FC = () => {
  const {environment} = useLocalnetRuntime()
  const endpoint = environment?.endpoints.observability
  const client = useMemo(
    () => createObservabilityClient(endpoint ?? "/unavailable-observability"),
    [endpoint],
  )

  return <SessionStats client={client} />
}
