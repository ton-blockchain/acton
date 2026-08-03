import type {FC} from "react"

import type {TonClient} from "@acton/explorer-core/api/client"
import {TokenCatalogPage} from "@acton/explorer-core/pages/TokenCatalogPage"

interface TokensPageProps {
  readonly client: TonClient
}

export const TokensPage: FC<TokensPageProps> = ({client}) => (
  <TokenCatalogPage client={client} embedded />
)
