import {Popover} from "../Popover"
import styles from "./ModeViewer.module.css"

export interface ModeReference {
  readonly name: string
  readonly value: number
}

export interface CodeReference {
  readonly code: string
  readonly href?: string
}

export type ModeDescription = string | readonly (string | ModeReference | CodeReference)[]

export interface ModeInfo {
  readonly name: string
  readonly value: number
  readonly description: ModeDescription
  readonly docsUrl?: string
}

export type ModeParser = (mode: number) => readonly ModeInfo[]

export interface ModeViewerProps {
  readonly mode: number | undefined
  readonly parseMode: ModeParser
}

function ModeConstant({mode}: {readonly mode: ModeReference}) {
  return (
    <span className={styles.constant}>
      {mode.name} ({mode.value})
    </span>
  )
}

function renderDescription(description: ModeDescription) {
  if (typeof description === "string") {
    return description
  }

  return description.map(part =>
    typeof part === "string" ? (
      part
    ) : "code" in part ? (
      part.href ? (
        <a
          key={`code-link-${part.code}`}
          href={part.href}
          target="_blank"
          rel="noreferrer"
          className={styles.codeLink}
        >
          <code className={styles.codeReference}>{part.code}</code>
        </a>
      ) : (
        <code key={`code-${part.code}`} className={styles.codeReference}>
          {part.code}
        </code>
      )
    ) : (
      <ModeConstant key={`mode-${part.name}-${part.value}`} mode={part} />
    ),
  )
}

export function ModeViewer({mode, parseMode}: ModeViewerProps) {
  if (mode === undefined) {
    return <span className={styles.empty}>No mode</span>
  }

  const flags = parseMode(mode)

  return (
    <span className={styles.container}>
      {flags.map((flag, index) => (
        <span key={`${flag.name}-${flag.value}`} className={styles.modeItem}>
          <Popover
            closeDelay={0}
            content={
              <span className={styles.popoverContent}>
                <span className={styles.popoverDescription}>
                  {renderDescription(flag.description)}
                </span>
                {flag.docsUrl ? (
                  <a
                    href={flag.docsUrl}
                    target="_blank"
                    rel="noreferrer"
                    className={styles.docsLink}
                    aria-label={`Read documentation for ${flag.name}`}
                  >
                    Documentation
                  </a>
                ) : undefined}
              </span>
            }
            placement="top"
            ariaLabel={`Explain ${flag.name}`}
          >
            <span className={styles.interactiveConstant}>
              <ModeConstant mode={flag} />
            </span>
          </Popover>
          {index === flags.length - 1 ? undefined : <span className={styles.plus}>+</span>}
        </span>
      ))}
    </span>
  )
}
