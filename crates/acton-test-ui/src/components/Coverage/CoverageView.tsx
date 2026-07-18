import {useCoverageReport} from "../../hooks/useCoverageReport"
import {Coverage} from "./Coverage"

import styles from "./Coverage.module.css"

interface CoverageViewProps {
  readonly projectRoot?: string
}

export function CoverageView({projectRoot}: CoverageViewProps) {
  const {lcov, error, loading} = useCoverageReport()

  if (loading) return <div className={styles.emptyState}>Loading coverage report...</div>
  if (error) return <div className={styles.emptyState}>Failed to load coverage: {error}</div>
  if (lcov === undefined) return <div className={styles.emptyState}>Coverage is not available</div>

  return <Coverage lcov={lcov} projectRoot={projectRoot} />
}
