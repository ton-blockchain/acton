import {useEffect, useState} from "react"
import type {FC, ImgHTMLAttributes} from "react"

import {NFT_PLACEHOLDER_IMAGE, deduplicateImageSources} from "./imageFallbacks"

interface NftImageProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "onError" | "src" | "srcSet"> {
  readonly sources: readonly string[]
  readonly blurred?: boolean
  readonly blurredClassName: string
}

interface ResolvedNftImage {
  readonly src: string
  readonly sourcesKey: string
}

/** Keeps local artwork visible until a candidate loads, so failed URLs never flash a broken image. */
export const NftImage: FC<NftImageProps> = ({
  sources,
  blurred = false,
  blurredClassName,
  className = "",
  alt = "",
  ...imageProps
}) => {
  const sourcesKey = deduplicateImageSources(sources).join("\u0000")
  const [image, setImage] = useState<ResolvedNftImage>()
  const loadedSource = image?.sourcesKey === sourcesKey ? image.src : undefined

  useEffect(() => {
    const imageSources = sourcesKey ? sourcesKey.split("\u0000") : []
    if (imageSources.length === 0) return

    const candidate = new Image()
    let sourceIndex = 0
    candidate.onload = () => setImage({src: imageSources[sourceIndex], sourcesKey})
    candidate.onerror = () => {
      sourceIndex += 1
      if (sourceIndex < imageSources.length) {
        candidate.src = imageSources[sourceIndex]
      }
    }
    candidate.src = imageSources[sourceIndex]

    // A previous NFT must not replace the current image after its metadata or route changes.
    return () => {
      candidate.onload = null
      candidate.onerror = null
    }
  }, [sourcesKey])

  return (
    <img
      {...imageProps}
      src={loadedSource ?? NFT_PLACEHOLDER_IMAGE}
      alt={alt}
      className={`${className}${loadedSource && blurred ? ` ${blurredClassName}` : ""}`}
      onError={event => {
        if (event.currentTarget.getAttribute("src") === NFT_PLACEHOLDER_IMAGE) return

        event.currentTarget.src = NFT_PLACEHOLDER_IMAGE
        setImage(undefined)
      }}
    />
  )
}
