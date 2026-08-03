import type {MouseEventHandler, ReactEventHandler, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {Tooltip} from "../Tooltip"

import styles from "./NftChip.module.css"

export interface NftChipProps {
  readonly ariaLabel?: string
  readonly className?: string
  readonly imageAlt?: string
  readonly imageSrc?: string
  readonly label: ReactNode
  readonly onClick?: MouseEventHandler<HTMLButtonElement>
  readonly onImageError?: ReactEventHandler<HTMLImageElement>
  readonly title?: string
}

export function NftChip({
  ariaLabel,
  className,
  imageAlt = "",
  imageSrc,
  label,
  onClick,
  onImageError,
  title,
}: NftChipProps) {
  const content = (
    <>
      {imageSrc && (
        <img
          className={styles.image}
          src={imageSrc}
          alt={imageAlt}
          loading="lazy"
          onError={onImageError}
        />
      )}
      <span className={styles.label}>{label}</span>
    </>
  )
  const chipClassName = cx(styles.nftChip, !imageSrc && styles.withoutImage, className)

  if (onClick) {
    return (
      <Tooltip content={title}>
        <button
          type="button"
          className={cx(chipClassName, styles.clickable)}
          aria-label={ariaLabel}
          onClick={onClick}
        >
          {content}
        </button>
      </Tooltip>
    )
  }

  return (
    <span className={chipClassName} title={title}>
      {content}
    </span>
  )
}
