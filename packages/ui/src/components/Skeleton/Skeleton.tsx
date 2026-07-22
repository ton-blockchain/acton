import type {ComponentPropsWithRef, CSSProperties} from "react"

import {cx} from "../../lib/cx"
import styles from "./Skeleton.module.css"

export type SkeletonShape = "circle" | "line" | "rect"
export type SkeletonRadius = "md" | "round" | "sm"

export type SkeletonProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children"> & {
    readonly animated?: boolean
    readonly height?: CSSProperties["height"]
    readonly radius?: SkeletonRadius
    readonly shape?: SkeletonShape
    readonly width?: CSSProperties["width"]
  }
>

export type SkeletonTextProps = Readonly<
  Omit<ComponentPropsWithRef<"div">, "children"> & {
    readonly animated?: boolean
    readonly lineCount?: number
    readonly lineHeight?: CSSProperties["height"]
    readonly widths?: readonly CSSProperties["width"][]
  }
>

const shapeClassNames = {
  circle: styles.shapeCircle,
  line: styles.shapeLine,
  rect: styles.shapeRect,
} satisfies Record<SkeletonShape, string>

const radiusClassNames = {
  md: styles.radiusMd,
  round: styles.radiusRound,
  sm: styles.radiusSm,
} satisfies Record<SkeletonRadius, string>

export function Skeleton({
  "aria-hidden": ariaHidden = true,
  animated = true,
  className,
  height,
  radius,
  ref,
  shape = "line",
  style,
  width,
  ...props
}: SkeletonProps) {
  return (
    <span
      {...props}
      ref={ref}
      aria-hidden={ariaHidden}
      data-animated={animated ? undefined : "false"}
      className={cx(
        styles.skeleton,
        shapeClassNames[shape],
        radius && radiusClassNames[radius],
        className,
      )}
      style={getSkeletonStyle(style, width, height)}
    />
  )
}

export function SkeletonText({
  "aria-hidden": ariaHidden = true,
  animated = true,
  className,
  lineCount = 3,
  lineHeight,
  ref,
  widths,
  ...props
}: SkeletonTextProps) {
  const count = Math.max(1, lineCount)

  return (
    <div
      {...props}
      ref={ref}
      aria-hidden={ariaHidden}
      className={cx(styles.skeletonText, className)}
    >
      {Array.from({length: count}).map((_, index) => (
        <Skeleton
          key={index}
          animated={animated}
          height={lineHeight}
          width={widths?.[index] ?? getDefaultLineWidth(index)}
        />
      ))}
    </div>
  )
}

function getSkeletonStyle(
  style: CSSProperties | undefined,
  width: CSSProperties["width"] | undefined,
  height: CSSProperties["height"] | undefined,
) {
  return {
    ...style,
    ...(width === undefined ? {} : {"--acton-skeleton-width": toCssSize(width)}),
    ...(height === undefined ? {} : {"--acton-skeleton-height": toCssSize(height)}),
  } as CSSProperties
}

function toCssSize(value: CSSProperties["width"]) {
  return typeof value === "number" ? `${value}px` : value
}

function getDefaultLineWidth(index: number) {
  if (index % 4 === 0) return "84%"
  if (index % 4 === 1) return "100%"
  if (index % 4 === 2) return "68%"
  return "92%"
}
