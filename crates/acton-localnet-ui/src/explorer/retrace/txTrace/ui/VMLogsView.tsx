import React from "react"

import {DataBlock} from "@acton/shared-ui"

export interface VMLogsViewProps {
  readonly logs: string | undefined
  readonly title?: string
  readonly isExpandable?: boolean
  readonly defaultExpanded?: boolean
}

const VMLogsView: React.FC<VMLogsViewProps> = ({
  logs,
  title = "Logs",
  isExpandable = false,
  defaultExpanded = false,
}) => {
  if (!logs) {
    return null
  }

  return (
    <DataBlock
      copyLabel={title}
      data={logs}
      defaultExpanded={defaultExpanded}
      label={title}
      maxHeight={isExpandable ? 420 : undefined}
      collapsible={isExpandable}
      variant="standalone"
      wrap={true}
    />
  )
}

export default VMLogsView
