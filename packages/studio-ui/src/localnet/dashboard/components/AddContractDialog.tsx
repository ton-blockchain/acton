import {Button, Dialog, Input, useToast} from "@acton/ui"
import {useEffect, useState} from "react"
import type {FormEvent} from "react"

import type {TonClient} from "@acton/explorer-core/api/client"
import {parseAddress, formatAddress} from "@acton/explorer-core/components/utils"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"

import styles from "./AddContractDialog.module.css"

interface AddContractDialogProps {
  readonly client: TonClient
  readonly open: boolean
  readonly onAdded: () => Promise<void>
  readonly onOpenChange: (open: boolean) => void
}

export function AddContractDialog({client, open, onAdded, onOpenChange}: AddContractDialogProps) {
  const {showToast} = useToast()
  const addressFormat = useAddressFormat()
  const [address, setAddress] = useState("")
  const [name, setName] = useState("")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (!open) {
      setAddress("")
      setName("")
      setSubmitting(false)
    }
  }, [open])

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault()

    const parsedAddress = parseAddress(address.trim())
    if (!parsedAddress) {
      showToast({
        title: "Contract not added",
        description: "Enter a valid TON address",
        variant: "error",
      })
      return
    }

    const contractAddress = formatAddress(parsedAddress.toRawString(), false, addressFormat)
    setSubmitting(true)
    try {
      const contractName = name.trim()
      await client.registerContract(contractAddress, contractName || undefined)
      await onAdded()
      onOpenChange(false)
      showToast({
        title: contractName ? `${contractName} added` : "Contract added",
        variant: "success",
      })
    } catch (error) {
      showToast({
        title: "Contract not added",
        description: error instanceof Error ? error.message : "Failed to add contract",
        variant: "error",
      })
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      open={open}
      title="Add contract"
      description="Add a deployed contract from this environment"
      maxWidth={520}
      onOpenChange={onOpenChange}
    >
      <form className={styles.form} onSubmit={handleSubmit}>
        <Input
          autoFocus
          label="Address"
          mono
          required
          value={address}
          placeholder="EQ… or 0:…"
          onChange={event => setAddress(event.target.value)}
        />
        <Input
          label="Name"
          description="Optional name shown throughout Studio"
          value={name}
          placeholder="Counter"
          onChange={event => setName(event.target.value)}
        />
        <div className={styles.actions}>
          <Button
            type="button"
            variant="secondary"
            disabled={submitting}
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={submitting} disabled={!address.trim()}>
            Add contract
          </Button>
        </div>
      </form>
    </Dialog>
  )
}
