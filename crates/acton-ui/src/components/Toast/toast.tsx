import {Toast as ToastBase, type ToastManagerUpdateOptions} from "@base-ui/react/toast"
import {CheckCircle2, CircleAlert, Info, LoaderCircle, X} from "lucide-react"
import {
  type ComponentPropsWithoutRef,
  createContext,
  type PropsWithChildren,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import styles from "./Toast.module.css"

export type ToastVariant = "info" | "success" | "error" | "loading"
export type ToastPriority = "low" | "high"

type ToastBaseOptions = Readonly<{
  readonly id?: string
  readonly variant?: ToastVariant
  readonly durationMs?: number
  readonly priority?: ToastPriority
}>

type ToastContentFields = Readonly<{
  readonly title?: ReactNode
  readonly description?: ReactNode
}>

type ToastRequiredContent =
  | Readonly<{
      readonly title: ReactNode
      readonly description?: ReactNode
    }>
  | Readonly<{
      readonly title?: ReactNode
      readonly description: ReactNode
    }>

export type ToastOptions = ToastBaseOptions & ToastRequiredContent

export type ToastUpdateOptions = Readonly<
  Partial<Omit<ToastBaseOptions, "id"> & ToastContentFields> & {
    readonly id?: never
  }
>

export type ToastPromiseOptions<Value> = Readonly<{
  readonly loading: ToastPromiseState<Value>
  readonly success: ToastPromiseState<Value>
  readonly error: ToastPromiseState<Value>
}>

export type ToastPromiseState<Value> =
  | string
  | ToastUpdateOptions
  | ((value: Value) => string | ToastUpdateOptions)

export type ToastContextValue = Readonly<{
  readonly showToast: (options: ToastOptions) => string
  readonly updateToast: (id: string, options: ToastUpdateOptions) => void
  readonly dismissToast: (id?: string) => void
  readonly promiseToast: <Value>(
    promise: Promise<Value>,
    options: ToastPromiseOptions<Value>,
  ) => Promise<Value>
}>

export type ToastProviderProps = PropsWithChildren<
  Readonly<{
    readonly timeoutMs?: number
    readonly limit?: number
    readonly viewportClassName?: string
  }>
>

type ToastData = Readonly<{
  variant?: ToastVariant
}>

const ToastContext = createContext<ToastContextValue | undefined>(undefined)
const defaultTimeoutMs = 4000
const defaultLimit = 4

export function ToastProvider({
  children,
  limit = defaultLimit,
  timeoutMs = defaultTimeoutMs,
  viewportClassName,
}: ToastProviderProps) {
  return (
    <ToastBase.Provider limit={limit} timeout={timeoutMs}>
      <ToastProviderBridge viewportClassName={viewportClassName}>{children}</ToastProviderBridge>
    </ToastBase.Provider>
  )
}

function ToastProviderBridge({
  children,
  viewportClassName,
}: PropsWithChildren<{
  readonly viewportClassName?: string
}>) {
  const {add, close, promise: trackPromise, update} = ToastBase.useToastManager<ToastData>()
  const {theme} = useTheme()

  const showToast = useCallback(
    (options: ToastOptions) => {
      return add(toBaseToastOptions(options, options.variant ?? "info"))
    },
    [add],
  )

  const updateToast = useCallback(
    (id: string, options: ToastUpdateOptions) => {
      update(id, toBaseToastOptions(options, options.variant))
    },
    [update],
  )

  const dismissToast = useCallback(
    (id?: string) => {
      close(id)
    },
    [close],
  )

  const promiseToast = useCallback(
    <Value,>(promise: Promise<Value>, options: ToastPromiseOptions<Value>) => {
      return trackPromise(promise, {
        loading: resolvePromiseState(options.loading, "loading"),
        success: result => resolvePromiseState(options.success, "success", result),
        error: error => resolvePromiseState(options.error, "error", error),
      })
    },
    [trackPromise],
  )

  const contextValue = useMemo<ToastContextValue>(
    () => ({
      dismissToast,
      promiseToast,
      showToast,
      updateToast,
    }),
    [dismissToast, promiseToast, showToast, updateToast],
  )

  return (
    <ToastContext.Provider value={contextValue}>
      {children}
      <ToastBase.Portal>
        <ToastBase.Viewport className={cx(styles.viewport, viewportClassName)} data-theme={theme}>
          <ToastList />
        </ToastBase.Viewport>
      </ToastBase.Portal>
    </ToastContext.Provider>
  )
}

function ToastList() {
  const {toasts} = ToastBase.useToastManager<ToastData>()

  return toasts.map(toast => {
    const variant = toast.data?.variant ?? normalizeToastVariant(toast.type)
    const layout = toast.title && toast.description ? "rich" : "single"

    return (
      <ToastBase.Root
        key={toast.id}
        toast={toast}
        className={styles.toast}
        data-variant={variant}
        data-layout={layout}
        swipeDirection={["right", "down"]}
      >
        <ToastBase.Content className={styles.content} data-layout={layout}>
          <span className={styles.icon} aria-hidden="true">
            <ToastIcon variant={variant} />
          </span>
          <span className={styles.body}>
            {toast.title ? <ToastBase.Title className={styles.title} /> : undefined}
            {toast.description ? (
              <ToastBase.Description className={styles.description} />
            ) : undefined}
          </span>
          <ToastBase.Close className={styles.closeButton} aria-label="Dismiss notification">
            <X size={16} strokeWidth={2.25} aria-hidden="true" />
          </ToastBase.Close>
        </ToastBase.Content>
      </ToastBase.Root>
    )
  })
}

function ToastIcon({variant}: {readonly variant: ToastVariant}) {
  if (variant === "success") return <CheckCircle2 size={17} strokeWidth={2.25} />
  if (variant === "error") return <CircleAlert size={17} strokeWidth={2.25} />
  if (variant === "loading") return <LoaderCircle size={17} strokeWidth={2.25} />
  return <Info size={17} strokeWidth={2.25} />
}

// react-doctor-disable-next-line react-doctor/only-export-components -- hook exports are safe Fast Refresh boundaries
export function useToast() {
  const context = useContext(ToastContext)

  if (!context) {
    throw new Error("useToast must be used within ToastProvider")
  }

  return context
}

function toBaseToastOptions(
  options: ToastOptions | ToastUpdateOptions,
  fallbackVariant: ToastVariant | undefined,
): ToastManagerUpdateOptions<ToastData> & {readonly id?: string} {
  const variant = options.variant ?? fallbackVariant
  const baseOptions: ToastManagerUpdateOptions<ToastData> & {id?: string} = {}

  if ("title" in options) {
    baseOptions.title = options.title
  }

  if ("description" in options) {
    baseOptions.description = options.description
  }

  if (options.durationMs !== undefined) {
    baseOptions.timeout = options.durationMs
  }

  if (options.priority !== undefined) {
    baseOptions.priority = options.priority
  } else if (variant === "error") {
    baseOptions.priority = "high"
  }

  if (variant !== undefined) {
    baseOptions.type = variant
    baseOptions.data = {variant}
  }

  if ("id" in options && options.id !== undefined) {
    baseOptions.id = options.id
  }

  return baseOptions
}

function resolvePromiseState<Value>(
  state: ToastPromiseState<Value>,
  variant: ToastVariant,
  value?: Value,
): ToastManagerUpdateOptions<ToastData> {
  const resolved = typeof state === "function" ? state(value as Value) : state

  if (typeof resolved === "string") {
    return toBaseToastOptions({title: resolved, variant}, variant)
  }

  return toBaseToastOptions({...resolved, variant: resolved.variant ?? variant}, variant)
}

function normalizeToastVariant(type: string | undefined): ToastVariant {
  if (type === "success" || type === "error" || type === "loading") return type
  return "info"
}

export type ToastCloseProps = ComponentPropsWithoutRef<typeof ToastBase.Close>
