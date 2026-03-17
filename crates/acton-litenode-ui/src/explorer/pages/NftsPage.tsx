import type React from "react"
import {useEffect, useState} from "react"
import {useNavigate} from "react-router-dom"

import type {TonClient} from "../api/client"
import type {NftItem} from "../api/types"
import {Nfts} from "../components/Nfts"

import styles from "./NftsPage.module.css"

interface NftsPageProps {
  readonly client: TonClient
}

export const NftsPage: React.FC<NftsPageProps> = ({client}) => {
  const navigate = useNavigate()
  const [items, setItems] = useState<NftItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  useEffect(() => {
    let isActive = true
    const load = async () => {
      setLoading(true)
      setError(undefined)
      try {
        const limit = 200
        let offset = 0
        const allItems: NftItem[] = []

        while (true) {
          const batch = await client.getNftItems({
            limit,
            offset,
            sortByLastTransactionLt: true,
          })
          allItems.push(...batch)
          if (batch.length < limit) {
            break
          }
          offset += limit
        }

        if (!isActive) return
        setItems([...new Map(allItems.map(item => [item.address, item])).values()])
      } catch (error) {
        if (!isActive) return
        setError(error instanceof Error ? error.message : "An error occurred")
      } finally {
        if (isActive) setLoading(false)
      }
    }

    void load()
    return () => {
      isActive = false
    }
  }, [client])

  const handleNftClick = (address: string) => {
    void navigate(`/explorer/address/${address}`)
  }

  return (
    <div className={styles.container}>
      <h1 className={styles.title}>NFT Items</h1>

      {loading && <div className={styles.loading}>Loading NFTs...</div>}
      {error && <div className={styles.error}>{error}</div>}

      {!loading && !error && <Nfts items={items} onAddressClick={handleNftClick} />}
    </div>
  )
}
