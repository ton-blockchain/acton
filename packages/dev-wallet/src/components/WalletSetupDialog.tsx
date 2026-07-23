import {Button, Dialog, Input, Select} from "@acton/ui"
import {KeyRound, Sparkles, WalletCards} from "lucide-react"
import {useEffect, useState, type FormEvent} from "react"

import type {WalletNetworkId, WalletVersion} from "../domain/wallet"
import styles from "./WalletSetupDialog.module.css"

export type WalletSetupMode = "create" | "import"

export interface WalletSetupSubmission {
  readonly name: string
  readonly network: WalletNetworkId
  readonly version: WalletVersion
  readonly mnemonic?: string
}

interface WalletSetupDialogProps {
  readonly mode?: WalletSetupMode
  readonly open: boolean
  readonly isSubmitting: boolean
  readonly onOpenChange: (open: boolean) => void
  readonly onSubmit: (submission: WalletSetupSubmission) => Promise<void>
}

export function WalletSetupDialog({
  mode: initialMode = "create",
  open,
  isSubmitting,
  onOpenChange,
  onSubmit,
}: WalletSetupDialogProps) {
  const [mode, setMode] = useState<WalletSetupMode>(initialMode)
  const [name, setName] = useState("Development wallet")
  const [network, setNetwork] = useState<WalletNetworkId>("testnet")
  const [version, setVersion] = useState<WalletVersion>("v5r1")
  const [mnemonic, setMnemonic] = useState("")

  useEffect(() => {
    if (open) {
      setMode(initialMode)
      setMnemonic("")
    }
  }, [initialMode, open])

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    await onSubmit({
      name,
      network,
      version,
      mnemonic: mode === "import" ? mnemonic : undefined,
    })
  }

  return (
    <Dialog
      open={open}
      title={mode === "create" ? "Create a developer wallet" : "Import a wallet"}
      description={
        mode === "create"
          ? "Choose the network and wallet contract you want to use for development."
          : "Enter the recovery phrase for the wallet you want to use here."
      }
      leadingIcon={mode === "create" ? <Sparkles size={20} /> : <KeyRound size={20} />}
      maxWidth={520}
      dismissible={!isSubmitting}
      onOpenChange={onOpenChange}
    >
      <form className={styles.form} onSubmit={event => void handleSubmit(event)}>
        <div className={styles.modeSwitch} aria-label="Wallet setup mode">
          <button
            type="button"
            className={mode === "create" ? styles.modeActive : undefined}
            onClick={() => setMode("create")}
          >
            New wallet
          </button>
          <button
            type="button"
            className={mode === "import" ? styles.modeActive : undefined}
            onClick={() => setMode("import")}
          >
            Import mnemonic
          </button>
        </div>

        <Input
          label="Wallet name"
          value={name}
          placeholder="Development wallet"
          leadingIcon={<WalletCards size={17} />}
          required={true}
          onChange={event => setName(event.target.value)}
        />

        <div className={styles.fieldGrid}>
          <Select
            label="Network"
            value={network}
            onChange={event => setNetwork(event.target.value as WalletNetworkId)}
          >
            <option value="testnet">Testnet</option>
            <option value="mainnet">Mainnet</option>
          </Select>
          <Select
            label="Contract"
            value={version}
            onChange={event => setVersion(event.target.value as WalletVersion)}
          >
            <option value="v5r1">Wallet V5R1</option>
            <option value="v4r2">Wallet V4R2</option>
          </Select>
        </div>

        {mode === "import" ? (
          <label className={styles.mnemonicField}>
            <span>Mnemonic</span>
            <textarea
              value={mnemonic}
              rows={5}
              spellCheck={false}
              autoCapitalize="off"
              autoComplete="off"
              placeholder="Enter 24 words separated by spaces"
              required={true}
              onChange={event => setMnemonic(event.target.value)}
            />
            <small>Check every word and its order before importing.</small>
          </label>
        ) : (
          <div className={styles.securityNote}>
            <KeyRound size={18} />
            <span>
              Write down the recovery phrase after creation. Anyone with the phrase can control this
              wallet.
            </span>
          </div>
        )}

        <div className={styles.actions}>
          <Button variant="ghost" disabled={isSubmitting} onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            type="submit"
            variant="primary"
            loading={isSubmitting}
            disabled={!name.trim() || (mode === "import" && !mnemonic.trim())}
          >
            {mode === "create" ? "Create wallet" : "Import wallet"}
          </Button>
        </div>
      </form>
    </Dialog>
  )
}
