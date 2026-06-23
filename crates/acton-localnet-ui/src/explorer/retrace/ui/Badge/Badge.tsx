import React from "react"

import styles from "./Badge.module.css"

interface BadgeProps {
  readonly color?: "green" | "red" | "blue" | "gray"
  readonly children: React.ReactNode
  readonly className?: string
}

const Badge: React.FC<BadgeProps> = ({color = "gray", children, className}) => {
  const colorClass = {
    green: styles.badgeGreen,
    red: styles.badgeRed,
    blue: styles.badgeBlue,
    gray: styles.badgeGray,
  }[color]

  return <span className={`${styles.badge} ${colorClass} ${className ?? ""}`}>{children}</span>
}

export default Badge
