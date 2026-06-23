import React from "react"

interface IconProps {
  readonly svg: React.ReactNode
  readonly size?: number
  readonly className?: string
  readonly ariaHidden?: boolean
}

const Icon: React.FC<IconProps> = ({svg, size = 16, className = "", ariaHidden = true}) => (
  <span
    className={className}
    style={{
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      width: size,
      height: size,
    }}
    aria-hidden={ariaHidden}
  >
    {svg}
  </span>
)
export default Icon
