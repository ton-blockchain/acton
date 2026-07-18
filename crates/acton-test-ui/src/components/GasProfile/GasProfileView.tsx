import {useGasProfileReport} from "../../hooks/useGasProfileReport"
import {GasProfile} from "./GasProfile"

import styles from "./GasProfile.module.css"

interface GasProfileViewProps {
  readonly projectRoot?: string
}

export function GasProfileView({projectRoot}: GasProfileViewProps) {
  const {profile, error, loading} = useGasProfileReport()

  if (loading) return <div className={styles.emptyState}>Loading gas profile...</div>
  if (error) return <div className={styles.emptyState}>Failed to load gas profile: {error}</div>
  if (profile === undefined) {
    return <div className={styles.emptyState}>Gas profile is not available</div>
  }

  return <GasProfile profile={profile} projectRoot={projectRoot} />
}
