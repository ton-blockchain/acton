import type {ComponentPropsWithRef, CSSProperties} from "react"
import {cx} from "../../lib/cx"
import styles from "./Slider.module.css"

export type SliderProps = Readonly<
  Omit<
    ComponentPropsWithRef<"input">,
    "type" | "value" | "defaultValue" | "onChange" | "min" | "max"
  > & {
    readonly value: number
    readonly min?: number
    readonly max?: number
    readonly onValueChange: (value: number) => void
  }
>

/** A single-value range with native keyboard and touch behavior */
export function Slider({
  value,
  min = 0,
  max = 100,
  onValueChange,
  className,
  style,
  ...props
}: SliderProps) {
  const progress = max > min ? Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100)) : 0

  return (
    <input
      {...props}
      type="range"
      min={min}
      max={max}
      value={value}
      onChange={event => onValueChange(event.currentTarget.valueAsNumber)}
      className={cx(styles.slider, className)}
      style={{...style, "--slider-progress": `${progress}%`} as CSSProperties}
    />
  )
}
