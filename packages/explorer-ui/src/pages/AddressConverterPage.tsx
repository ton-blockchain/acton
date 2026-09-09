import {useNavigate, useSearchParams} from "react-router"
import {AddressConverterPage as Converter} from "@acton/explorer-core/pages/AddressConverterPage"
import {useNetworkInfo} from "@acton/explorer-core/hooks/useNetworkInfo"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"

/** Opens converted addresses using the API client and URL for their selected public network. */
export function AddressConverterPage() {
  const {network} = useNetworkInfo()
  const routes = useExplorerRoutePaths()
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()

  return (
    <Converter
      onOpenAddress={(address, testOnly) => {
        const search = new URLSearchParams(searchParams)
        search.delete("address")
        let targetNetwork = network.id
        if (testOnly !== undefined) targetNetwork = testOnly ? "testnet" : "mainnet"
        search.set("network", targetNetwork)
        const pathname = routes.addressPath(address)
        // Switching networks must reinitialize the app's API client from the destination URL.
        if (targetNetwork !== network.id) {
          globalThis.location.assign(`${pathname}?${search}`)
          return
        }
        void navigate({pathname, search: search.toString()})
      }}
    />
  )
}
