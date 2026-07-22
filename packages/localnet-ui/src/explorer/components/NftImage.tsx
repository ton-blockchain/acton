import {useEffect, useRef, useState} from "react"
import type {FC, ImgHTMLAttributes, SyntheticEvent} from "react"

import {isRegisteredNsfwNft} from "../nftSafetyRegistry"

import {TOKEN_PLACEHOLDER_IMAGE} from "./imageFallbacks"

interface NftImageProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "onError" | "src" | "srcSet"> {
  readonly sources: readonly string[]
  readonly blurred?: boolean
  readonly blurredClassName: string
  readonly collectionName?: string
  readonly onNsfw?: () => void
}

interface ResolvedNftImage {
  readonly src: string
  readonly blurred: boolean
  readonly hidden: boolean
  readonly verified: boolean
}

const toHex = (value: ArrayBuffer): string =>
  Array.from(new Uint8Array(value), byte => byte.toString(16).padStart(2, "0")).join("")

const getInitialImage = (
  sources: readonly string[],
  blurred: boolean,
  collectionName: string | undefined,
): ResolvedNftImage => {
  const primarySource = sources[0]
  if (!primarySource) {
    return {src: TOKEN_PLACEHOLDER_IMAGE, blurred: false, hidden: false, verified: true}
  }

  const registered = sources.some(imageUrl => isRegisteredNsfwNft({imageUrl, collectionName}))
  if (registered) {
    return {src: TOKEN_PLACEHOLDER_IMAGE, blurred: false, hidden: true, verified: true}
  }
  if (blurred) {
    return {src: primarySource, blurred: true, hidden: false, verified: false}
  }

  return {src: TOKEN_PLACEHOLDER_IMAGE, blurred: false, hidden: false, verified: true}
}

export const NftImage: FC<NftImageProps> = ({
  sources,
  blurred = false,
  blurredClassName,
  collectionName,
  onNsfw,
  className = "",
  alt = "",
  ...imageProps
}) => {
  const sourcesKey = sources.join("\u0000")
  const onNsfwRef = useRef(onNsfw)
  onNsfwRef.current = onNsfw
  const [image, setImage] = useState<ResolvedNftImage>(() =>
    getInitialImage(sources, blurred, collectionName),
  )

  useEffect(() => {
    const imageSources = sourcesKey ? sourcesKey.split("\u0000") : []
    const immediateImage = getInitialImage(imageSources, blurred, collectionName)
    setImage(immediateImage)

    if (immediateImage.hidden) {
      onNsfwRef.current?.()
      return
    }

    if (
      imageSources.length === 0 ||
      immediateImage.blurred ||
      immediateImage.src !== TOKEN_PLACEHOLDER_IMAGE
    ) {
      return
    }

    const controller = new AbortController()
    let objectUrl: string | undefined

    void (async () => {
      for (const source of imageSources) {
        try {
          const response = await fetch(source, {signal: controller.signal})
          if (!response.ok) {
            continue
          }

          const bytes = await response.arrayBuffer()
          const digest = await globalThis.crypto.subtle.digest("SHA-256", bytes)
          const contentHash = toHex(digest)
          const blob = new Blob([bytes], {
            type: response.headers.get("content-type") || "application/octet-stream",
          })
          objectUrl = URL.createObjectURL(blob)

          if (controller.signal.aborted) {
            URL.revokeObjectURL(objectUrl)
            objectUrl = undefined
            return
          }

          if (isRegisteredNsfwNft({contentHash, collectionName})) {
            URL.revokeObjectURL(objectUrl)
            objectUrl = undefined
            setImage({
              src: TOKEN_PLACEHOLDER_IMAGE,
              blurred: false,
              hidden: true,
              verified: true,
            })
            onNsfwRef.current?.()
            return
          }

          setImage({src: objectUrl, blurred: false, hidden: false, verified: true})
          return
        } catch {
          if (controller.signal.aborted) {
            return
          }
        }
      }

      // If the browser cannot inspect an unknown image (for example because of CORS),
      // render it blurred instead of briefly exposing unverified content.
      setImage({
        src: imageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE,
        blurred: true,
        hidden: false,
        verified: false,
      })
    })()

    return () => {
      controller.abort()
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl)
      }
    }
  }, [blurred, collectionName, sourcesKey])

  const handleImageError = (event: SyntheticEvent<HTMLImageElement>) => {
    if (image.verified) {
      setImage({
        src: TOKEN_PLACEHOLDER_IMAGE,
        blurred: false,
        hidden: false,
        verified: true,
      })
      return
    }

    const imageSources = sourcesKey ? sourcesKey.split("\u0000") : []
    const currentSource = event.currentTarget.getAttribute("src")
    const currentIndex = currentSource ? imageSources.indexOf(currentSource) : -1
    const nextSource = imageSources[currentIndex + 1]
    setImage({
      src: nextSource ?? TOKEN_PLACEHOLDER_IMAGE,
      blurred: nextSource !== undefined,
      hidden: false,
      verified: false,
    })
  }

  if (image.hidden) {
    return null
  }

  return (
    <img
      {...imageProps}
      src={image.src}
      alt={alt}
      className={`${className}${image.blurred ? ` ${blurredClassName}` : ""}`}
      onError={handleImageError}
    />
  )
}
