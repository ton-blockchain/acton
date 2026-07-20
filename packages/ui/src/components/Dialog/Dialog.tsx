import {Dialog as DialogBase} from "@base-ui/react/dialog"
import {X} from "lucide-react"
import type {CSSProperties, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import styles from "./Dialog.module.css"

export interface DialogProps {
  readonly children: ReactNode
  readonly className?: string
  readonly closeLabel?: string
  readonly contentClassName?: string
  readonly description?: ReactNode
  readonly maxWidth?: CSSProperties["maxWidth"]
  readonly onOpenChange: (open: boolean) => void
  readonly open: boolean
  readonly title: ReactNode
}

export function Dialog({
  children,
  className,
  closeLabel = "Close dialog",
  contentClassName,
  description,
  maxWidth,
  onOpenChange,
  open,
  title,
}: DialogProps) {
  const {theme} = useTheme()

  return (
    <DialogBase.Root open={open} onOpenChange={onOpenChange}>
      <DialogBase.Portal>
        <DialogBase.Backdrop className={styles.backdrop} data-theme={theme} />
        <DialogBase.Viewport className={styles.viewport}>
          <DialogBase.Popup
            className={cx(styles.popup, className)}
            data-theme={theme}
            style={
              maxWidth === undefined
                ? undefined
                : ({"--acton-dialog-max-width": toCssSize(maxWidth)} as CSSProperties)
            }
          >
            <header className={styles.header}>
              <div className={styles.heading}>
                <DialogBase.Title className={styles.title}>{title}</DialogBase.Title>
                {description !== undefined && description !== null && (
                  <DialogBase.Description className={styles.description}>
                    {description}
                  </DialogBase.Description>
                )}
              </div>
              <DialogBase.Close className={styles.closeButton} aria-label={closeLabel}>
                <X size={18} aria-hidden="true" />
              </DialogBase.Close>
            </header>
            <div className={cx(styles.content, contentClassName)}>{children}</div>
          </DialogBase.Popup>
        </DialogBase.Viewport>
      </DialogBase.Portal>
    </DialogBase.Root>
  )
}

function toCssSize(value: CSSProperties["maxWidth"]) {
  return typeof value === "number" ? `${value}px` : value
}
