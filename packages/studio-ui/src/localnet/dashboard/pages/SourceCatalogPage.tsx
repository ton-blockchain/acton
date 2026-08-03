import type {TonClient} from "@acton/explorer-core/api/client"
import {SourceCatalog} from "@acton/explorer-core/components/SourceCatalog"

export function SourceCatalogPage({client}: {readonly client: TonClient}) {
  return <SourceCatalog client={client} />
}
