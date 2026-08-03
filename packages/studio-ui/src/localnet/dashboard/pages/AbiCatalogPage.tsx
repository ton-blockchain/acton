import {AbiCatalog, AbiDetails, useAbiDetails} from "@acton/explorer-core/components/AbiCatalog"
import {useParams} from "react-router"

export function AbiCatalogPage() {
  return <AbiCatalog />
}

export function AbiDetailsPage() {
  const {slug = ""} = useParams()
  const state = useAbiDetails(slug)

  return <AbiDetails state={state} />
}
