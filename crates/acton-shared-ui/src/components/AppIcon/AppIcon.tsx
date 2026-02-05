import type React from "react"

interface AppIconProps {
  readonly theme: string
  readonly size?: number
}

export const AppIcon: React.FC<AppIconProps> = ({ theme, size = 28 }) => {
  if (theme === "dark") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 16 16"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        role="img"
        aria-label="Acton Tests app icon"
      >
        <g clipPath="url(#clip0_18_3209)">
          <path
            d="M12 1H4C2.34315 1 1 2.34315 1 4V12C1 13.6569 2.34315 15 4 15H12C13.6569 15 15 13.6569 15 12V4C15 2.34315 13.6569 1 12 1Z"
            fill="url(#paint0_linear_18_3209)"
          />
          <path
            d="M12 1H4C2.34315 1 1 2.34315 1 4V12C1 13.6569 2.34315 15 4 15H12C13.6569 15 15 13.6569 15 12V4C15 2.34315 13.6569 1 12 1Z"
            fill="url(#paint1_linear_18_3209)"
          />
          <mask
            id="mask0_18_3209"
            style={{ maskType: "luminance" }}
            maskUnits="userSpaceOnUse"
            x="0"
            y="0"
            width="16"
            height="16"
          >
            <path d="M16 0H0V16H16V0Z" fill="black" />
            <path
              d="M8 12.25C10.2091 12.25 12 10.4591 12 8.25C12 6.04086 10.2091 4.25 8 4.25C5.79086 4.25 4 6.04086 4 8.25C4 10.4591 5.79086 12.25 8 12.25Z"
              fill="white"
            />
            <path
              d="M9.625 10.5C9.625 11.125 9.125 11.625 8.5 11.625H6.75C5.625 11.625 4.75 10.75 4.75 9.625C4.75 8.5 5.625 7.625 6.75 7.625H9.625V10.5ZM9.625 6.375H6.75C5 6.375 3.5 7.875 3.5 9.625C3.5 11.375 5 12.875 6.75 12.875H8.5C9.625 12.875 10.625 11.875 10.625 10.75V6.375H9.625Z"
              fill="black"
            />
          </mask>
          <g mask="url(#mask0_18_3209)">
            <path d="M16 0H0V16H16V0Z" fill="white" />
          </g>
        </g>
        <defs>
          <linearGradient
            id="paint0_linear_18_3209"
            x1="1"
            y1="1"
            x2="15"
            y2="15"
            gradientUnits="userSpaceOnUse"
          >
            <stop stopColor="#222329" />
            <stop offset="1" stopColor="#0B0B0F" />
          </linearGradient>
          <linearGradient
            id="paint1_linear_18_3209"
            x1="1"
            y1="1"
            x2="1"
            y2="15"
            gradientUnits="userSpaceOnUse"
          >
            <stop stopOpacity="0.14" />
            <stop offset="0.45" stopOpacity="0.04" />
            <stop offset="1" stopOpacity="0" />
          </linearGradient>
          <clipPath id="clip0_18_3209">
            <rect width="16" height="16" fill="white" />
          </clipPath>
        </defs>
      </svg>
    )
  }

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="Acton Tests app icon"
    >
      <g clip-path="url(#clip0_22_3097)">
        <path
          d="M12 1H4C2.34315 1 1 2.34315 1 4V12C1 13.6569 2.34315 15 4 15H12C13.6569 15 15 13.6569 15 12V4C15 2.34315 13.6569 1 12 1Z"
          fill="url(#paint0_linear_22_3097)"
        />
        <path
          d="M12 1H4C2.34315 1 1 2.34315 1 4V12C1 13.6569 2.34315 15 4 15H12C13.6569 15 15 13.6569 15 12V4C15 2.34315 13.6569 1 12 1Z"
          fill="url(#paint1_radial_22_3097)"
        />
        <path
          d="M12 1H4C2.34315 1 1 2.34315 1 4V12C1 13.6569 2.34315 15 4 15H12C13.6569 15 15 13.6569 15 12V4C15 2.34315 13.6569 1 12 1Z"
          fill="url(#paint2_radial_22_3097)"
        />
        <mask
          id="mask0_22_3097"
          style={{ maskType: "luminance" }}
          maskUnits="userSpaceOnUse"
          x="0"
          y="0"
          width="16"
          height="16"
        >
          <path d="M16 0H0V16H16V0Z" fill="black" />
          <path
            d="M8 12.25C10.2091 12.25 12 10.4591 12 8.25C12 6.04086 10.2091 4.25 8 4.25C5.79086 4.25 4 6.04086 4 8.25C4 10.4591 5.79086 12.25 8 12.25Z"
            fill="white"
          />
          <path
            d="M9.625 10.5C9.625 11.125 9.125 11.625 8.5 11.625H6.75C5.625 11.625 4.75 10.75 4.75 9.625C4.75 8.5 5.625 7.625 6.75 7.625H9.625V10.5ZM9.625 6.375H6.75C5 6.375 3.5 7.875 3.5 9.625C3.5 11.375 5 12.875 6.75 12.875H8.5C9.625 12.875 10.625 11.875 10.625 10.75V6.375H9.625Z"
            fill="black"
          />
        </mask>
        <g mask="url(#mask0_22_3097)">
          <path d="M16 0H0V16H16V0Z" fill="url(#paint3_linear_22_3097)" />
        </g>
        <mask
          id="mask1_22_3097"
          style={{ maskType: "luminance" }}
          maskUnits="userSpaceOnUse"
          x="0"
          y="0"
          width="16"
          height="16"
        >
          <path d="M16 0H0V16H16V0Z" fill="black" />
          <path
            d="M8 12.25C10.2091 12.25 12 10.4591 12 8.25C12 6.04086 10.2091 4.25 8 4.25C5.79086 4.25 4 6.04086 4 8.25C4 10.4591 5.79086 12.25 8 12.25Z"
            fill="white"
          />
          <path
            d="M9.625 10.5C9.625 11.125 9.125 11.625 8.5 11.625H6.75C5.625 11.625 4.75 10.75 4.75 9.625C4.75 8.5 5.625 7.625 6.75 7.625H9.625V10.5ZM9.625 6.375H6.75C5 6.375 3.5 7.875 3.5 9.625C3.5 11.375 5 12.875 6.75 12.875H8.5C9.625 12.875 10.625 11.875 10.625 10.75V6.375H9.625Z"
            fill="black"
          />
        </mask>
        <g mask="url(#mask1_22_3097)">
          <path d="M16 0H0V16H16V0Z" fill="url(#paint4_radial_22_3097)" />
        </g>
        <path
          d="M8 12.25C10.2091 12.25 12 10.4591 12 8.25C12 6.04086 10.2091 4.25 8 4.25C5.79086 4.25 4 6.04086 4 8.25C4 10.4591 5.79086 12.25 8 12.25Z"
          stroke="white"
          stroke-opacity="0.18"
        />
      </g>
      <defs>
        <linearGradient
          id="paint0_linear_22_3097"
          x1="1"
          y1="1"
          x2="15"
          y2="15"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="white" />
          <stop offset="1" stop-color="#E9EAEE" />
        </linearGradient>
        <radialGradient
          id="paint1_radial_22_3097"
          cx="0"
          cy="0"
          r="1"
          gradientUnits="userSpaceOnUse"
          gradientTransform="translate(5.75 4.5) rotate(45) scale(13.75)"
        >
          <stop stop-color="white" stop-opacity="0.85" />
          <stop offset="0.55" stop-color="white" stop-opacity="0.25" />
          <stop offset="1" stop-color="white" stop-opacity="0" />
        </radialGradient>
        <radialGradient
          id="paint2_radial_22_3097"
          cx="0"
          cy="0"
          r="1"
          gradientUnits="userSpaceOnUse"
          gradientTransform="translate(8 8) rotate(90) scale(10.625)"
        >
          <stop stop-opacity="0" />
          <stop offset="1" stop-opacity="0.06" />
        </radialGradient>
        <linearGradient
          id="paint3_linear_22_3097"
          x1="5.75"
          y1="4.25"
          x2="10.75"
          y2="12.25"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#2B2C31" />
          <stop offset="1" stop-color="#0B0B0F" />
        </linearGradient>
        <radialGradient
          id="paint4_radial_22_3097"
          cx="0"
          cy="0"
          r="1"
          gradientUnits="userSpaceOnUse"
          gradientTransform="translate(6.75 6) rotate(45) scale(7.5)"
        >
          <stop stop-color="white" stop-opacity="0.22" />
          <stop offset="0.55" stop-color="white" stop-opacity="0.06" />
          <stop offset="1" stop-color="white" stop-opacity="0" />
        </radialGradient>
        <clipPath id="clip0_22_3097">
          <rect width="16" height="16" fill="white" />
        </clipPath>
      </defs>
    </svg>
  )
}
