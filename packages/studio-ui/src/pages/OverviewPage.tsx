import {ArrowRight, Radio, Sparkles} from "lucide-react"
import type {MouseEvent} from "react"

import type {StudioConnectionState} from "../studioApi"
import type {StudioPage, StudioPath} from "../studioPages"

import styles from "./OverviewPage.module.css"

interface OverviewPageProps {
  readonly connectionState: StudioConnectionState
  readonly pages: readonly StudioPage[]
  readonly projectName?: string
  readonly projectPath?: string
  readonly onNavigate: (path: StudioPath) => void
}

export function OverviewPage({
  connectionState,
  pages,
  projectName,
  projectPath,
  onNavigate,
}: OverviewPageProps) {
  const featurePages = pages.filter(page => page.path !== "/")
  const connectionLabel =
    connectionState === "connected"
      ? "Connected"
      : connectionState === "connecting"
        ? "Connecting"
        : "Not connected"
  const connectionDescription =
    connectionState === "connected"
      ? "Studio server is available"
      : connectionState === "connecting"
        ? "Looking for Studio server"
        : "Start Studio with acton studio start"
  const connectionDotClass =
    connectionState === "connected"
      ? styles.statusDotConnected
      : connectionState === "disconnected"
        ? styles.statusDotDisconnected
        : ""
  const workspaceDescription =
    projectPath ??
    (connectionState === "connected"
      ? projectName
        ? "Project open"
        : "No project selected"
      : connectionState === "connecting"
        ? "Connecting to Studio server"
        : "Waiting for Studio server")

  const navigateFromAnchor = (event: MouseEvent<HTMLAnchorElement>, path: StudioPath) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return
    }

    event.preventDefault()
    onNavigate(path)
  }

  return (
    <div className={styles.page}>
      <section className={styles.signalStrip} aria-label="Workspace status">
        <div className={styles.signal}>
          <strong>{projectName || "No project open"}</strong>
          <small className={projectPath ? styles.technicalValue : undefined}>
            {workspaceDescription}
          </small>
        </div>
        <div className={styles.signal}>
          <strong className={styles.signalValue}>
            <span className={`${styles.statusDot} ${connectionDotClass}`} />
            {connectionLabel}
          </strong>
          <small>{connectionDescription}</small>
        </div>
        <div className={styles.signal}>
          <strong className={styles.signalValue}>
            <Sparkles size={15} aria-hidden="true" />
            Local first
          </strong>
          <small>Cloud account is not required</small>
        </div>
      </section>

      <div className={styles.contentGrid}>
        <section className={styles.workspaceSection} aria-labelledby="workspace-tools-title">
          <div className={styles.sectionHeader}>
            <h2 id="workspace-tools-title">Tools</h2>
          </div>

          <div className={styles.toolList}>
            {featurePages.map(page => {
              const Icon = page.icon

              return (
                <a
                  key={page.path}
                  href={page.path}
                  className={styles.toolRow}
                  onClick={event => navigateFromAnchor(event, page.path)}
                >
                  <span className={styles.toolIcon}>
                    <Icon size={19} aria-hidden="true" />
                  </span>
                  <span className={styles.toolCopy}>
                    <strong>{page.label}</strong>
                    <small>{page.shortDescription}</small>
                  </span>
                  <span className={styles.toolState}>
                    {page.path === "/virtual-environments"
                      ? "No instances"
                      : page.path === "/simulator"
                        ? "No sessions"
                        : "No runs"}
                  </span>
                  <ArrowRight size={17} aria-hidden="true" className={styles.toolArrow} />
                </a>
              )
            })}
          </div>
        </section>

        <section className={styles.activitySection} aria-labelledby="activity-title">
          <div className={styles.sectionHeader}>
            <h2 id="activity-title">Recent activity</h2>
          </div>

          <div className={styles.emptyActivity}>
            <span className={styles.activityIcon}>
              <Radio size={19} aria-hidden="true" />
            </span>
            <div>
              <strong>Your activity will appear here</strong>
              <p>Runs, simulations and environment events will share one timeline</p>
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
