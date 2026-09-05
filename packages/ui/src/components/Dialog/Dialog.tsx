import {Dialog as DialogBase} from "@base-ui/react/dialog"
import {X} from "lucide-react"
import type {ComponentPropsWithoutRef, CSSProperties, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import styles from "./Dialog.module.css"

export interface DialogProps {
  readonly busy?: boolean
  readonly children: ReactNode
  readonly className?: string
  readonly closeLabel?: string
  readonly contentClassName?: string
  readonly contentPadding?: "default" | "none"
  readonly description?: ReactNode
  readonly dismissible?: boolean
  /** Pins actions below a scrollable header and body, keeping them reachable on short screens */
  readonly footer?: ReactNode
  /** Changes header composition without replacing the accessible title and description */
  readonly headerClassName?: string
  readonly leadingIcon?: ReactNode
  readonly maxWidth?: CSSProperties["maxWidth"]
  readonly onOpenChange: (open: boolean) => void
  readonly open: boolean
  readonly title: ReactNode
}

export function Dialog({
  busy = false,
  children,
  className,
  closeLabel = "Close dialog",
  contentClassName,
  contentPadding = "default",
  description,
  dismissible = true,
  footer,
  headerClassName,
  leadingIcon,
  maxWidth,
  onOpenChange,
  open,
  title,
}: DialogProps) {
  const {theme} = useTheme()
  const canDismiss = dismissible && !busy
  const handleOpenChange = (nextOpen: boolean) => {
    if (nextOpen || canDismiss) {
      onOpenChange(nextOpen)
    }
  }

  const content = (
    <>
      <header className={cx(styles.header, headerClassName)}>
        {leadingIcon !== undefined && leadingIcon !== null && (
          <div className={styles.leadingIcon}>{leadingIcon}</div>
        )}
        <div className={styles.heading}>
          <DialogBase.Title className={styles.title}>{title}</DialogBase.Title>
          {description !== undefined && description !== null && (
            <DialogBase.Description className={styles.description}>
              {description}
            </DialogBase.Description>
          )}
        </div>
        {canDismiss && (
          <DialogBase.Close className={styles.closeButton} aria-label={closeLabel}>
            <X size={18} aria-hidden="true" />
          </DialogBase.Close>
        )}
      </header>
      <div
        className={cx(
          styles.content,
          contentPadding === "none" && styles.contentPaddingNone,
          contentClassName,
        )}
      >
        {children}
      </div>
    </>
  )

  return (
    <DialogBase.Root open={open} onOpenChange={handleOpenChange}>
      <DialogBase.Portal>
        <DialogBase.Backdrop className={styles.backdrop} data-theme={theme} />
        <DialogBase.Viewport className={styles.viewport}>
          <DialogBase.Popup
            className={cx(styles.popup, className)}
            data-theme={theme}
            aria-busy={busy || undefined}
            style={
              maxWidth === undefined
                ? undefined
                : ({"--acton-dialog-max-width": toCssSize(maxWidth)} as CSSProperties)
            }
          >
            {footer === undefined || footer === null ? (
              content
            ) : (
              <>
                <div className={styles.scrollArea}>{content}</div>
                {footer}
              </>
            )}
          </DialogBase.Popup>
        </DialogBase.Viewport>
      </DialogBase.Portal>
    </DialogBase.Root>
  )
}

export interface DialogActionsProps extends ComponentPropsWithoutRef<"footer"> {
  readonly stackOnMobile?: boolean
}

export function DialogActions({
  children,
  className,
  stackOnMobile = false,
  ...props
}: DialogActionsProps) {
  return (
    <footer
      className={cx(styles.actions, stackOnMobile && styles.actionsStackOnMobile, className)}
      {...props}
    >
      {children}
    </footer>
  )
}

function toCssSize(value: CSSProperties["maxWidth"]) {
  return typeof value === "number" ? `${value}px` : value
}
