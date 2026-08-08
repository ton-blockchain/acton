import {Button, Dialog} from "@acton/ui"
import {Trash2} from "lucide-react"

import type {StudioEnvironment} from "../../../studioApi"

import styles from "./DeleteEnvironmentDialog.module.css"

interface DeleteEnvironmentDialogProps {
  readonly environment: StudioEnvironment | undefined
  readonly loading: boolean
  readonly onConfirm: () => void
  readonly onOpenChange: (open: boolean) => void
}

export function DeleteEnvironmentDialog({
  environment,
  loading,
  onConfirm,
  onOpenChange,
}: DeleteEnvironmentDialogProps) {
  return (
    <Dialog
      open={environment !== undefined}
      onOpenChange={onOpenChange}
      title={environment ? `Delete ${environment.name}` : "Delete environment"}
      description="This permanently deletes the environment and all of its saved data"
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
          leadingIcon={<Trash2 size={15} aria-hidden="true" />}
          onClick={onConfirm}
        >
          Delete environment
        </Button>
      </div>
    </Dialog>
  )
}
