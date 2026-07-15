import type {ReactNode} from "react"

export type GalleryNote = Readonly<{
  title: string
  items: readonly string[]
}>

export type GallerySection = Readonly<{
  id: string
  title: string
  description?: string
  content: ReactNode
}>

type GalleryPage = Readonly<{
  id: string
  title: string
  status: string
  summary: string
  sections: readonly GallerySection[]
}>

export type ComponentGallery = GalleryPage &
  (
    | Readonly<{
        kind?: "component"
        importStatement: string
        agentSummary: string
        usage: readonly string[]
        avoid: readonly string[]
      }>
    | Readonly<{
        kind: "foundation"
      }>
  )
