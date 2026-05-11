"use client"

import {Play} from "lucide-react"
import type {ComponentPropsWithoutRef} from "react"
import {useEffect, useRef, useState} from "react"
import {cn} from "@/lib/cn"

interface LandingVideoProps extends ComponentPropsWithoutRef<"video"> {
  initialVolume?: number
  playLabel?: string
}

export function LandingVideo({
  className,
  children,
  controls,
  initialVolume,
  playLabel = "Play video",
  ...props
}: LandingVideoProps) {
  const videoRef = useRef<HTMLVideoElement | null>(null)
  const shouldFocusAfterStart = useRef(false)
  const [hasStarted, setHasStarted] = useState(false)

  useEffect(() => {
    if (initialVolume === undefined) {
      return
    }

    const video = videoRef.current

    if (!video) {
      return
    }

    video.volume = Math.min(Math.max(initialVolume, 0), 1)
  }, [initialVolume])

  useEffect(() => {
    if (!hasStarted || !shouldFocusAfterStart.current) {
      return
    }

    shouldFocusAfterStart.current = false
    videoRef.current?.focus({preventScroll: true})
  }, [hasStarted])

  const playVideo = () => {
    const video = videoRef.current

    if (!video) {
      return
    }

    shouldFocusAfterStart.current = true
    setHasStarted(true)
    void video.play()
  }

  return (
    <div className="landing-video-frame">
      <video
        ref={videoRef}
        className={cn("landing-video", className)}
        controls={controls ? hasStarted : undefined}
        tabIndex={hasStarted ? 0 : -1}
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
          <span className="landing-video-play-surface">
            <Play
              aria-hidden="true"
              className="landing-video-play-icon"
              fill="currentColor"
              strokeWidth={2.4}
            />
          </span>
        </button>
      ) : null}
    </div>
  )
}
