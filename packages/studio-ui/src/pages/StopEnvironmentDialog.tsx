import {Button, Dialog} from "@acton/ui"
import {Square} from "lucide-react"

import type {StudioEnvironment} from "../studioApi"

import styles from "./StopEnvironmentDialog.module.css"

interface StopEnvironmentDialogProps {
  readonly environment: StudioEnvironment | undefined
  readonly loading: boolean
  readonly onConfirm: () => void
  readonly onOpenChange: (open: boolean) => void
}

export function StopEnvironmentDialog({
  environment,
  loading,
  onConfirm,
  onOpenChange,
}: StopEnvironmentDialogProps) {
  return (
    <Dialog
      open={environment !== undefined}
      onOpenChange={onOpenChange}
      title={environment ? `Stop ${environment.name}` : "Stop environment"}
      description="The RPC endpoint will remain unavailable until you restart this environment"
      dismissible={!loading}
      maxWidth="28rem"
    >
      <div className={styles.actions}>
        <Button
          type="button"
          variant="secondary"
          disabled={loading}
          onClick={() => onOpenChange(false)}
        >
          Cancel
        </Button>
        <Button
          type="button"
          variant="danger"
          loading={loading}
          leadingIcon={<Square size={14} aria-hidden="true" />}
          onClick={onConfirm}
        >
          Stop environment
        </Button>
      </div>
    </Dialog>
  )
}
