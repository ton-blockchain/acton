import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import styles from "./VisuallyGroupedNumber.module.css"

const VISUAL_NUMBER_PATTERN = /^(-?)(\d+)(\.\d+)?$/

export type VisuallyGroupedNumberProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children"> & {
    readonly value: string | number | bigint
  }
>

function splitVisualNumberGroups(value: string): readonly string[] | undefined {
  const match = VISUAL_NUMBER_PATTERN.exec(value)
  if (!match) return undefined

  const [, sign, integerPart, fractionPart = ""] = match
  if (integerPart.length <= 3) return undefined

  const firstGroupLength = integerPart.length % 3 || 3
  const groups = [`${sign}${integerPart.slice(0, firstGroupLength)}`]

  for (let start = firstGroupLength; start < integerPart.length; start += 3) {
    groups.push(integerPart.slice(start, start + 3))
  }

  groups[groups.length - 1] += fractionPart
  return groups
}

export function VisuallyGroupedNumber({value, className, ...props}: VisuallyGroupedNumberProps) {
  const normalizedValue = String(value)
  const groups = splitVisualNumberGroups(normalizedValue)

  if (!groups) {
    return (
      <span {...props} className={className}>
        {normalizedValue}
      </span>
    )
  }

  return (
    <span {...props} className={cx(styles.number, className)}>
      {groups.map((group, index) => (
        <span key={`${index}-${group}`} className={index === 0 ? undefined : styles.group}>
          {group}
        </span>
      ))}
    </span>
  )
}
