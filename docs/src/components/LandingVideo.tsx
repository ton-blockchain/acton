"use client"

import {Play} from "lucide-react"
import type {ComponentPropsWithoutRef} from "react"
import {useRef, useState} from "react"
import {cn} from "@/lib/cn"

interface LandingVideoProps extends ComponentPropsWithoutRef<"video"> {
  playLabel?: string
}

export function LandingVideo({
  className,
  children,
  controls,
  playLabel = "Play video",
  ...props
}: LandingVideoProps) {
  const videoRef = useRef<HTMLVideoElement | null>(null)
  const [hasStarted, setHasStarted] = useState(false)

  const playVideo = () => {
    const video = videoRef.current

    if (!video) {
      return
    }

    setHasStarted(true)
    void video.play()
  }

  return (
    <div className="landing-video-frame">
      <video
        ref={videoRef}
        className={cn("landing-video", className)}
        controls={controls ? hasStarted : undefined}
        {...props}
      >
        {children}
      </video>
      {!hasStarted ? (
        <button
          type="button"
          className="landing-video-play-button"
          aria-label={playLabel}
          onClick={playVideo}
        >
          <Play
            aria-hidden="true"
            className="landing-video-play-icon"
            fill="currentColor"
            strokeWidth={2.4}
          />
        </button>
      ) : null}
    </div>
  )
}
