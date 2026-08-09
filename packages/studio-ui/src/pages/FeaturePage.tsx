import {Button} from "@acton/ui"

import type {StudioFeaturePage} from "../studioPages"

import styles from "./FeaturePage.module.css"

interface FeaturePageProps {
  readonly page: StudioFeaturePage
  readonly onAction: (action: string) => void
}

export function FeaturePage({page, onAction}: FeaturePageProps) {
  const Icon = page.icon

  return (
    <div className={styles.page}>
      <section className={styles.emptyPanel} aria-labelledby="feature-empty-title">
        <span className={styles.emptyIcon}>
          <Icon size={24} aria-hidden="true" />
        </span>
        <h2 id="feature-empty-title">{page.emptyTitle}</h2>
        <p>{page.emptyDescription}</p>
        <Button variant="secondary" size="sm" onClick={() => onAction(page.actionLabel)}>
          {page.actionLabel}
        </Button>
      </section>
    </div>
  )
}
