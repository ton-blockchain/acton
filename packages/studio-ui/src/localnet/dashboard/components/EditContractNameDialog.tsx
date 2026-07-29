import {Button, Dialog, Input, useToast} from "@acton/ui"
import {useEffect, useState} from "react"
import type {FormEvent} from "react"

import type {TonClient} from "@acton/explorer-core/api/client"
import type {LocalnetContract} from "@acton/explorer-core/api/types"

import styles from "./AddContractDialog.module.css"

interface EditContractNameDialogProps {
  readonly client: TonClient
  readonly contract: LocalnetContract | undefined
  readonly onOpenChange: (open: boolean) => void
  readonly onSaved: () => Promise<void>
}

export function EditContractNameDialog({
  client,
  contract,
  onOpenChange,
  onSaved,
}: EditContractNameDialogProps) {
  const {showToast} = useToast()
  const [name, setName] = useState("")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    setName(contract?.name ?? "")
    setSubmitting(false)
  }, [contract])

  const nameUnchanged = name.trim() === (contract?.name?.trim() ?? "")

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault()
    if (!contract) return

    const nextName = name.trim()
    setSubmitting(true)
    try {
      await client.setAddressName(contract.address, nextName)
      await onSaved()
      onOpenChange(false)
      showToast({
        title: nextName ? "Contract name updated" : "Custom name removed",
        variant: "success",
      })
    } catch (error) {
      showToast({
        title: "Contract name not updated",
        description: error instanceof Error ? error.message : "Failed to update contract name",
        variant: "error",
      })
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      open={contract !== undefined}
      title="Edit contract name"
      description="Set the name shown throughout Studio"
      maxWidth={460}
      onOpenChange={onOpenChange}
    >
      <form className={styles.form} onSubmit={handleSubmit}>
        <Input
          autoFocus
          label="Name"
          value={name}
          placeholder={contract?.abiName ?? "Counter"}
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
          <Button type="submit" variant="primary" loading={submitting} disabled={nameUnchanged}>
            Save name
          </Button>
        </div>
      </form>
    </Dialog>
  )
}
