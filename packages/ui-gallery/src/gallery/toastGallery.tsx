import {Button, InlineButton, MarkdownText, useToast} from "@acton/ui"
import {CheckCircle2, CircleAlert, Info, LoaderCircle, RefreshCw} from "lucide-react"
import {useRef} from "react"

import styles from "./toastGallery.module.css"
import type {ComponentGallery} from "./types"

const iconProps = {
  size: 16,
  strokeWidth: 2.25,
} as const

function VariantSamples() {
  const {showToast} = useToast()

  return (
    <div className={styles.grid}>
      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Info</h4>
          <p>Neutral feedback for copied values, background sync, and non-blocking notices.</p>
        </div>
        <Button
          variant="secondary"
          leadingIcon={<Info {...iconProps} />}
          onClick={() =>
            showToast({
              title: "Raw body copied",
              description: "The base64 body is available from your clipboard.",
              variant: "info",
            })
          }
        >
          Show info
        </Button>
      </article>

      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Success</h4>
          <p>Confirmation after an action completed and no follow-up is required.</p>
        </div>
        <Button
          variant="secondary"
          leadingIcon={<CheckCircle2 {...iconProps} />}
          onClick={() =>
            showToast({
              title: "Session approved",
              description: "The dapp can now use the selected startup wallet.",
              variant: "success",
            })
          }
        >
          Show success
        </Button>
      </article>

      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Error</h4>
          <p>Failure feedback with urgent screen-reader priority by default.</p>
        </div>
        <Button
          variant="secondary"
          leadingIcon={<CircleAlert {...iconProps} />}
          onClick={() =>
            showToast({
              title: "Trace replay failed",
              description: "The sandbox could not reproduce the transaction state.",
              variant: "error",
            })
          }
        >
          Show error
        </Button>
      </article>
    </div>
  )
}

function UpdatingSample() {
  const {dismissToast, showToast, updateToast} = useToast()
  const loadingToastIdRef = useRef<string | undefined>(undefined)

  const startRefresh = () => {
    if (loadingToastIdRef.current) {
      dismissToast(loadingToastIdRef.current)
    }

    const toastId = showToast({
      title: "Refreshing wallets",
      description: "Fetching sessions and balances from the local node.",
      variant: "loading",
    })

    loadingToastIdRef.current = toastId

    globalThis.setTimeout(() => {
      updateToast(toastId, {
        title: "Wallets refreshed",
        description: "Startup wallets, sessions, and balances are up to date.",
        variant: "success",
        durationMs: 3500,
      })
      loadingToastIdRef.current = undefined
    }, 3000)
  }

  return (
    <div className={styles.panel}>
      <div className={styles.panelText}>
        <h4>Refresh flow</h4>
        <MarkdownText tone="muted">
          {
            "Use `showToast` for the loading state, then `updateToast` with the same id when the async work completes."
          }
        </MarkdownText>
      </div>
      <Button variant="primary" leadingIcon={<RefreshCw {...iconProps} />} onClick={startRefresh}>
        Refresh for 3 seconds
      </Button>
    </div>
  )
}

function CompactMessageSamples() {
  const {showToast} = useToast()

  return (
    <div className={styles.panel}>
      <div className={styles.panelText}>
        <h4>Compact messages</h4>
        <p>Use title-only feedback when no supporting description is needed.</p>
      </div>
      <div className={styles.buttonRow}>
        <Button
          variant="secondary"
          leadingIcon={<CheckCircle2 {...iconProps} />}
          onClick={() =>
            showToast({
              title: "Source deleted",
              variant: "success",
            })
          }
        >
          Title only
        </Button>
      </div>
    </div>
  )
}

function PromiseSample() {
  const {promiseToast} = useToast()

  const runPromise = () => {
    void promiseToast(
      new Promise<string>(resolve => {
        globalThis.setTimeout(() => resolve("EQB8YtZZA7Kzz3cH8B36"), 3000)
      }),
      {
        loading: {
          title: "Resolving trace path",
          description: "Known breadcrumb segments stay visible while the missing segment loads.",
        },
        success: address => ({
          title: "Trace path resolved",
          description: (
            <>
              Last segment resolved to <code>{address}</code>.
            </>
          ),
        }),
        error: "Trace path could not be resolved.",
      },
    )
  }

  return (
    <div className={styles.panel}>
      <div className={styles.panelText}>
        <h4>Promise helper</h4>
        <MarkdownText tone="muted">
          {
            "`promiseToast` keeps loading, success, and error text in one place when a workflow already returns a promise."
          }
        </MarkdownText>
      </div>
      <Button
        variant="secondary"
        leadingIcon={<LoaderCircle {...iconProps} />}
        onClick={runPromise}
      >
        Resolve promise
      </Button>
    </div>
  )
}

function RichContentSample() {
  const {showToast} = useToast()

  return (
    <div className={styles.inlinePanel}>
      <span className={styles.inlineLabel}>Rich content is allowed in descriptions.</span>
      <InlineButton
        variant="accent"
        leadingIcon={<Info size={14} strokeWidth={2.25} />}
        onClick={() =>
          showToast({
            title: "TON Connect request",
            description: (
              <>
                The request can be inspected in{" "}
                <a href="https://docs.ton.org/" rel="noreferrer" target="_blank">
                  TON docs
                </a>
                .
              </>
            ),
            variant: "info",
          })
        }
      >
        Show rich toast
      </InlineButton>
    </div>
  )
}

export const toastGallery = {
  id: "toast",
  title: "Toast",
  status: "ready",
  summary:
    "Toast renders temporary feedback for completed actions, recoverable errors, and async workflow state.",
  importStatement: 'import {ToastProvider, useToast} from "@acton/ui"',
  agentSummary:
    "Wrap an app once in ToastProvider, then call useToast from action handlers. Use updateToast or promiseToast for long async work instead of creating separate loading and success notifications.",
  usage: [
    "Use for non-blocking feedback after user actions such as copy, refresh, approve, reject, and disconnect.",
    'Use `variant="loading"` for work in progress, then update the same toast id when the work completes.',
    "Keep toast text short; include details only when the user can act on them immediately.",
    'Use `variant="error"` for recoverable failures, not validation messages that belong next to a field.',
  ],
  avoid: [
    "Do not use Toast for confirmations that block a destructive decision.",
    "Do not create a second toast when an existing loading toast should be updated.",
    "Do not put long logs, forms, or table content inside a toast.",
  ],
  sections: [
    {
      id: "toast-variants",
      title: "Variants",
      description: "Info, success, and error feedback use the same layout with different emphasis.",
      content: <VariantSamples />,
    },
    {
      id: "toast-updating",
      title: "Updating Toast",
      description: "A loading toast can be updated in place when a refresh finishes.",
      content: <UpdatingSample />,
    },
    {
      id: "toast-compact",
      title: "Compact Messages",
      description: "Single-line notifications do not reserve empty space for missing content.",
      content: <CompactMessageSamples />,
    },
    {
      id: "toast-promise",
      title: "Promise Flow",
      description: "Promise helpers keep async feedback consistent without manual timers.",
      content: <PromiseSample />,
    },
    {
      id: "toast-rich-content",
      title: "Rich Description",
      description: "Descriptions may include compact inline links or technical values.",
      content: <RichContentSample />,
    },
  ],
} satisfies ComponentGallery
