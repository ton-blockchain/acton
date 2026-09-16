import {Banner} from "@acton/ui"
import {Code2, ExternalLink} from "lucide-react"
import {useState} from "react"
import {useNetworkInfo} from "@acton/explorer-core/hooks/useNetworkInfo"

const DISMISSED_STORAGE_KEY = "actonExplorerDeveloperBannerDismissed"

const isDismissed = (): boolean => {
  try {
    return localStorage.getItem(DISMISSED_STORAGE_KEY) === "true"
  } catch {
    return false
  }
}

export function DeveloperExplorerBanner() {
  const {network} = useNetworkInfo()
  const [visible, setVisible] = useState(() => !isDismissed())

  if (!visible) {
    return null
  }

  const dismiss = () => {
    try {
      localStorage.setItem(DISMISSED_STORAGE_KEY, "true")
    } catch {
      // The banner can still be dismissed when storage is unavailable.
    }
    setVisible(false)
  }

  return (
    <Banner
      aria-label="Developer explorer notice"
      icon={<Code2 size={16} />}
      title="Acton Explorer is made for smart-contract developers"
      compactTitle="Acton Explorer is made for developers"
      description="Looking for a public TON explorer?"
      action={
        <a
          href={network.id === "mainnet" ? "https://tonscan.org" : "https://testnet.tonscan.org"}
          target="_blank"
          rel="noreferrer"
        >
          Visit Tonscan
          <ExternalLink size={13} aria-hidden="true" />
        </a>
      }
      dismissLabel="Dismiss developer explorer notice"
      onDismiss={dismiss}
    />
  )
}
