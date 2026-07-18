import {Popover as PopoverBase} from "@base-ui/react/popover"
import {type ComponentPropsWithRef, type ReactNode, type Ref, useCallback, useState} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import styles from "./Popover.module.css"

export type PopoverPlacement = "top" | "right" | "bottom" | "left"
export type PopoverInteraction = "hover" | "click"

export type PopoverProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children" | "content"> & {
    readonly children: ReactNode
    readonly content: ReactNode
    readonly interaction?: PopoverInteraction
    readonly placement?: PopoverPlacement
    readonly open?: boolean
    readonly defaultOpen?: boolean
    readonly onOpenChange?: (open: boolean) => void
    readonly openDelay?: number
    readonly closeDelay?: number
    readonly offset?: number
    readonly contentClassName?: string
    readonly triggerClassName?: string
    readonly panelId?: string
    readonly ariaLabel?: string
  }
>

const defaultOffset = 8

export function Popover({
  children,
  className,
  closeDelay = 120,
  content,
  contentClassName,
  defaultOpen = false,
  interaction = "hover",
  offset = defaultOffset,
  onOpenChange,
  open,
  openDelay = 0,
  panelId,
  placement = "bottom",
  ref,
  tabIndex = 0,
  triggerClassName,
  ariaLabel,
  ...props
}: PopoverProps) {
  const {theme} = useTheme()
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen)
  const isControlled = open !== undefined
  const isOpen = open ?? uncontrolledOpen

  const setTriggerRef = useCallback(
    (node: HTMLSpanElement | null) => {
      assignRef(ref, node)
    },
    [ref],
  )

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!isControlled) setUncontrolledOpen(nextOpen)
      onOpenChange?.(nextOpen)
    },
    [isControlled, onOpenChange],
  )

  return (
    <PopoverBase.Root open={isOpen} onOpenChange={handleOpenChange}>
      <PopoverBase.Trigger
        closeDelay={closeDelay}
        delay={openDelay}
        nativeButton={false}
        openOnHover={interaction === "hover"}
        render={
          <span
            {...props}
            ref={setTriggerRef}
            tabIndex={tabIndex}
            className={cx(styles.popover, className)}
            data-interaction={interaction}
          />
        }
      >
        <span className={cx(styles.trigger, triggerClassName)}>{children}</span>
      </PopoverBase.Trigger>

      <PopoverBase.Portal>
        <PopoverBase.Positioner className={styles.positioner} side={placement} sideOffset={offset}>
          <PopoverBase.Popup
            id={panelId}
            aria-label={ariaLabel}
            className={cx(styles.panel, contentClassName)}
            data-theme={theme}
          >
            <PopoverBase.Arrow className={styles.arrow}>
              <ArrowSvg />
            </PopoverBase.Arrow>
            {content}
          </PopoverBase.Popup>
        </PopoverBase.Positioner>
      </PopoverBase.Portal>
    </PopoverBase.Root>
  )
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null) {
  if (!ref) return

  if (typeof ref === "function") {
    ref(value)
    return
  }

  ref.current = value
}

function ArrowSvg(props: ComponentPropsWithRef<"svg">) {
  return (
    <svg width="20" height="10" viewBox="0 0 20 10" fill="none" {...props}>
      <path
        d="M9.66 2.6L4.81 6.97C4.07 7.63 3.12 8 2.13 8H0V10H20V8H18.53C17.55 8 16.59 7.63 15.86 6.97L11 2.6C10.62 2.26 10.04 2.26 9.66 2.6Z"
        className={styles.arrowBody}
      />
      <path
        d="M9 1.86C9.76 1.17 10.91 1.17 11.67 1.86L16.53 6.23C17.08 6.73 17.79 7 18.53 7L15.89 7L11 2.6C10.62 2.26 10.04 2.26 9.66 2.6L4.78 7L2.13 7C2.87 7 3.59 6.73 4.14 6.23L9 1.86Z"
        className={styles.arrowOuterStroke}
      />
      <path
        d="M10.33 3.35L5.48 7.72C4.56 8.54 3.37 9 2.13 9H0V8H2.13C3.12 8 4.07 7.63 4.81 6.97L9.66 2.6C10.04 2.26 10.62 2.26 11 2.6L15.86 6.97C16.59 7.63 17.55 8 18.53 8H20V9H18.53C17.3 9 16.11 8.54 15.19 7.72L10.33 3.35Z"
        className={styles.arrowInnerStroke}
      />
    </svg>
  )
}
