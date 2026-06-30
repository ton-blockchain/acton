import type React from "react"

const VISUAL_NUMBER_PATTERN = /^(-?)(\d+)(\.\d+)?$/
const VISUAL_NUMBER_GROUP_STYLE: React.CSSProperties = {
  marginLeft: "0.28em",
}

function splitVisualNumberGroups(value: string): readonly string[] | undefined {
  const match = VISUAL_NUMBER_PATTERN.exec(value)
  if (!match) {
    return undefined
  }

  const [, sign, integerPart, fractionPart = ""] = match
  if (integerPart.length <= 3) {
    return undefined
  }

  const firstGroupLength = integerPart.length % 3 || 3
  const groups = [`${sign}${integerPart.slice(0, firstGroupLength)}`]

  for (let start = firstGroupLength; start < integerPart.length; start += 3) {
    groups.push(integerPart.slice(start, start + 3))
  }

  groups[groups.length - 1] += fractionPart
  return groups
}

export function VisuallyGroupedNumber({
  value,
  className,
}: {
  readonly value: string
  readonly className?: string
}): React.JSX.Element {
  const groups = splitVisualNumberGroups(value)
  if (!groups) {
    return <span className={className}>{value}</span>
  }

  return (
    <span className={className}>
      {groups.map((group, index) => (
        <span key={`${index}-${group}`} style={index === 0 ? undefined : VISUAL_NUMBER_GROUP_STYLE}>
          {group}
        </span>
      ))}
    </span>
  )
}
