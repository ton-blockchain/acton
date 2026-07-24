import type React from "react"
import {
  GasProfile as SharedGasProfile,
  type GasProfileData,
  type GasProfileReport,
} from "@acton/transaction-ui"

import {useGasProfileReport} from "../../hooks/useGasProfileReport"

export type {GasProfileData, GasProfileReport, GasProfileTestReport} from "@acton/transaction-ui"

interface GasProfileProps {
  readonly profile?: GasProfileData
  readonly projectRoot?: string
}

export const GasProfile: React.FC<GasProfileProps> = ({profile, projectRoot}) => {
  const {profile: loadedProfile, error, loading} = useGasProfileReport(profile === undefined)

  if (profile !== undefined) {
    return <SharedGasProfile profile={profile} projectRoot={projectRoot} />
  }
  if (loading) return <div>Loading gas profile...</div>
  if (error) return <div>Failed to load gas profile: {error}</div>
  if (loadedProfile === undefined) return <div>Gas profile is not available</div>

  return (
    <SharedGasProfile
      profile={loadedProfile satisfies GasProfileReport}
      projectRoot={projectRoot}
    />
  )
}
