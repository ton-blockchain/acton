import type {ComponentPropsWithRef, KeyboardEvent, ReactNode} from "react"
import {useEffect, useId, useRef, useState} from "react"

import {cx} from "../../lib/cx"
import {SkeletonText} from "../Skeleton"
import styles from "./ContentTabs.module.css"

export type ContentTab<TValue extends string = string> = Readonly<{
  readonly disabled?: boolean
  readonly label: ReactNode
  readonly value: TValue
}>

export type ContentTabsProps<TValue extends string = string> = Readonly<
  Omit<ComponentPropsWithRef<"div">, "children" | "onChange"> & {
    readonly ariaLabel?: string
    readonly children: ReactNode
    readonly listClassName?: string
    readonly loading?: boolean
    readonly loadingFallback?: ReactNode
    readonly loadingLabel?: string
    readonly loadingValue?: TValue
    readonly onValueChange: (value: TValue) => void | Promise<unknown>
    readonly panelClassName?: string
    readonly tabs: readonly ContentTab<TValue>[]
    readonly value: TValue
  }
>

type PromiseSelection<TValue extends string> = Readonly<{
  requestId: number
  status: "loading" | "settled"
  value: TValue
}>

export function ContentTabs<TValue extends string = string>({
  ariaLabel,
  children,
  className,
  id,
  listClassName,
  loading = false,
  loadingFallback,
  loadingLabel = "Loading tab content",
  loadingValue,
  onValueChange,
  panelClassName,
  ref,
  tabs,
  value,
  ...props
}: ContentTabsProps<TValue>) {
  const generatedId = useId()
  const loadingRequestIdRef = useRef(0)
  const [promiseSelection, setPromiseSelection] = useState<PromiseSelection<TValue> | undefined>()
  const baseId = id ?? generatedId
  const panelId = `${baseId}-panel`
  const selectedValue = promiseSelection?.value ?? (loading ? loadingValue : undefined) ?? value
  const isPromiseLoading =
    promiseSelection?.status === "loading" ||
    (promiseSelection?.status === "settled" && value !== promiseSelection.value)
  const isLoading = loading || isPromiseLoading
  const activeIndex = tabs.findIndex(tab => tab.value === selectedValue)
  const activeTabId = activeIndex >= 0 ? getTabId(baseId, activeIndex) : undefined

  // react-doctor-disable-next-line react-doctor/no-reset-all-state-on-prop-change -- clears completed async selection tracking
  useEffect(() => {
    if (!promiseSelection) return
    if (value !== promiseSelection.value || promiseSelection.status === "loading") return

    setPromiseSelection(undefined)
  }, [promiseSelection, value])

  const focusTab = (index: number) => {
    if (typeof document === "undefined") return

    globalThis.requestAnimationFrame(() => {
      document.getElementById(getTabId(baseId, index))?.focus()
    })
  }

  const activateEnabledTab = (enabledIndex: number) => {
    const enabledTabs = getEnabledTabs(tabs)
    const nextTab = enabledTabs[enabledIndex]

    if (!nextTab) return

    activateTab(nextTab.tab.value)
    focusTab(nextTab.index)
  }

  const handleTabListKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const enabledTabs = getEnabledTabs(tabs)
    if (enabledTabs.length === 0) return

    const currentEnabledIndex = Math.max(
      0,
      enabledTabs.findIndex(item => item.tab.value === selectedValue),
    )
    const lastEnabledIndex = enabledTabs.length - 1
    let nextEnabledIndex: number | undefined

    if (event.key === "ArrowRight") {
      nextEnabledIndex = currentEnabledIndex === lastEnabledIndex ? 0 : currentEnabledIndex + 1
    } else if (event.key === "ArrowLeft") {
      nextEnabledIndex = currentEnabledIndex === 0 ? lastEnabledIndex : currentEnabledIndex - 1
    } else if (event.key === "Home") {
      nextEnabledIndex = 0
    } else if (event.key === "End") {
      nextEnabledIndex = lastEnabledIndex
    }

    if (nextEnabledIndex === undefined) return

    event.preventDefault()
    activateEnabledTab(nextEnabledIndex)
  }

  return (
    <div {...props} ref={ref} id={id} className={cx(styles.contentTabs, className)}>
      <div
        className={cx(styles.tabList, listClassName)}
        role="tablist"
        aria-label={ariaLabel}
        onKeyDown={handleTabListKeyDown}
      >
        {tabs.map((tab, index) => {
          const isActive = tab.value === selectedValue

          return (
            <button
              key={tab.value}
              type="button"
              id={getTabId(baseId, index)}
              className={cx(styles.tab, isActive && styles.tabActive)}
              role="tab"
              aria-controls={panelId}
              aria-selected={isActive}
              aria-busy={isLoading && isActive ? true : undefined}
              disabled={tab.disabled}
              tabIndex={isActive ? 0 : -1}
              onClick={() => activateTab(tab.value)}
            >
              {tab.label}
            </button>
          )
        })}
      </div>

      <div
        id={panelId}
        className={cx(styles.panel, panelClassName)}
        role="tabpanel"
        aria-labelledby={activeTabId}
        aria-busy={isLoading || undefined}
      >
        {isLoading ? (
          <>
            <span className={styles.loadingStatus} role="status">
              {loadingLabel}
            </span>
            {loadingFallback ?? <SkeletonText className={styles.loadingSkeleton} lineCount={8} />}
          </>
        ) : (
          children
        )}
      </div>
    </div>
  )

  function activateTab(nextValue: TValue) {
    const result = onValueChange(nextValue)

    if (!isPromiseLike(result)) {
      loadingRequestIdRef.current += 1
      setPromiseSelection(undefined)
      return
    }

    const requestId = loadingRequestIdRef.current + 1
    loadingRequestIdRef.current = requestId
    setPromiseSelection({requestId, status: "loading", value: nextValue})

    void result.then(
      () => settlePromiseSelection(requestId),
      () => clearPromiseSelection(requestId),
    )
  }

  function settlePromiseSelection(requestId: number) {
    setPromiseSelection(current => {
      if (!current || current.requestId !== requestId) return current
      return {...current, status: "settled"}
    })
  }

  function clearPromiseSelection(requestId: number) {
    setPromiseSelection(current => {
      if (!current || current.requestId !== requestId) return current
      return undefined
    })
  }
}

function getTabId(baseId: string, index: number) {
  return `${baseId}-tab-${index}`
}

function getEnabledTabs<TValue extends string>(tabs: readonly ContentTab<TValue>[]) {
  // react-doctor-disable-next-line react-doctor/js-combine-iterations -- tab lists are small and the chain is clearer
  return tabs.map((tab, index) => ({index, tab})).filter(item => !item.tab.disabled)
}

function isPromiseLike(value: unknown): value is Promise<unknown> {
  return (
    (typeof value === "object" || typeof value === "function") &&
    value !== null &&
    "then" in value &&
    typeof value.then === "function"
  )
}
