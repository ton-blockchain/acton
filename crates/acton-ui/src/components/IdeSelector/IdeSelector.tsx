import {Menu} from "@base-ui/react/menu"
import {Check, ChevronDown} from "lucide-react"
import {type ComponentPropsWithoutRef, useCallback, useEffect, useState} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import {IdeIcon} from "./icons"
import styles from "./IdeSelector.module.css"

export const IDE_OPTIONS = [
  {id: "Cursor", label: "Cursor"},
  {id: "Windsurf", label: "Windsurf"},
  {id: "VS Code", label: "VS Code"},
  {id: "VSCodium", label: "VSCodium"},
  {id: "WebStorm", label: "WebStorm"},
  {id: "RustRover", label: "RustRover"},
  {id: "IntelliJ", label: "IntelliJ IDEA"},
] as const

export type IdeId = (typeof IDE_OPTIONS)[number]["id"]

export type IdeLocation = Readonly<{
  readonly filePath: string
  readonly line: number
  readonly column: number
}>

export type IdeSelectorProps = Readonly<
  Omit<ComponentPropsWithoutRef<"div">, "children" | "onChange"> & {
    readonly location: IdeLocation
    readonly onValueChange: (ide: IdeId) => void
    readonly shortcut?: boolean
    readonly size?: "compact" | "default"
    readonly value: IdeId
  }
>

export const DEFAULT_IDE: IdeId = "Cursor"

export function isIdeId(value: unknown): value is IdeId {
  return typeof value === "string" && IDE_OPTIONS.some(option => option.id === value)
}

export function getIdeUrl(ide: IdeId, location: IdeLocation): string {
  const {column, filePath, line} = location

  switch (ide) {
    case "Cursor": {
      return `cursor://file/${filePath}:${line}:${column}`
    }
    case "Windsurf": {
      return `windsurf://file/${filePath}:${line}:${column}`
    }
    case "VS Code": {
      return `vscode://file/${filePath}:${line}:${column}`
    }
    case "VSCodium": {
      return `vscodium://file/${filePath}:${line}:${column}`
    }
    case "WebStorm": {
      return `webstorm://open?file=${filePath}&line=${line}&column=${column}`
    }
    case "RustRover": {
      return `rustrover://open?file=${filePath}&line=${line}&column=${column}`
    }
    case "IntelliJ": {
      return `idea://open?file=${filePath}&line=${line}&column=${column}`
    }
  }
}

export function getIdeLabel(ide: IdeId): string {
  return IDE_OPTIONS.find(option => option.id === ide)?.label ?? ide
}

export function useIdePreference(storageKey = "selectedIde") {
  const [ide, setIde] = useState<IdeId>(() => {
    try {
      const savedIde = globalThis.localStorage?.getItem(storageKey)
      return isIdeId(savedIde) ? savedIde : DEFAULT_IDE
    } catch {
      return DEFAULT_IDE
    }
  })

  const selectIde = useCallback(
    (nextIde: IdeId) => {
      setIde(nextIde)
      try {
        globalThis.localStorage?.setItem(storageKey, nextIde)
      } catch {
        // The selector still works when storage is unavailable.
      }
    },
    [storageKey],
  )

  return [ide, selectIde] as const
}

export function IdeSelector({
  className,
  location,
  onValueChange,
  shortcut = false,
  size = "default",
  value,
  ...props
}: IdeSelectorProps) {
  const {theme} = useTheme()
  const {column, filePath, line} = location
  const label = getIdeLabel(value)
  const url = getIdeUrl(value, {column, filePath, line})

  useEffect(() => {
    if (!shortcut) return

    const handleShortcut = (event: KeyboardEvent) => {
      if (
        event.key !== "." ||
        event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        isEditableTarget(event.target)
      ) {
        return
      }

      event.preventDefault()
      globalThis.location.href = getIdeUrl(value, {column, filePath, line})
    }

    globalThis.addEventListener("keydown", handleShortcut)
    return () => globalThis.removeEventListener("keydown", handleShortcut)
  }, [column, filePath, line, shortcut, value])

  return (
    <div
      {...props}
      className={cx(styles.selector, size === "compact" && styles.compact, className)}
    >
      <a
        href={url}
        className={styles.quickLink}
        aria-label={`Open in ${label}`}
        title={`Open in ${label}${shortcut ? " (or press `.`)" : ""}`}
      >
        <IdeIcon ide={value} />
      </a>

      <Menu.Root modal={false}>
        <Menu.Trigger
          type="button"
          className={styles.trigger}
          aria-label="Change IDE"
          title="Change IDE"
        >
          <ChevronDown aria-hidden="true" />
        </Menu.Trigger>

        <Menu.Portal>
          <Menu.Positioner className={styles.positioner} sideOffset={4} align="end">
            <Menu.Popup className={styles.popup} data-theme={theme} aria-label="Choose IDE">
              <Menu.RadioGroup
                value={value}
                onValueChange={nextValue => onValueChange(nextValue as IdeId)}
              >
                {IDE_OPTIONS.map(option => (
                  <Menu.RadioItem
                    key={option.id}
                    value={option.id}
                    label={option.label}
                    closeOnClick
                    className={styles.item}
                  >
                    <span className={styles.itemIcon}>
                      <IdeIcon ide={option.id} />
                    </span>
                    <span>{option.label}</span>
                    <Check className={styles.check} aria-hidden="true" />
                  </Menu.RadioItem>
                ))}
              </Menu.RadioGroup>
            </Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
    </div>
  )
}

function isEditableTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement)
  )
}
