import {docs} from "collections/server"
import {findPath} from "fumadocs-core/page-tree"
import {loader} from "fumadocs-core/source"
import {icons} from "lucide-react"
import {createElement} from "react"
import {type InferPageType} from "fumadocs-core/source"

export const source = loader({
  baseUrl: "/docs",
  source: docs.toFumadocsSource(),
  icon(icon) {
    if (!icon) {
      return
    }
    if (icon in icons) {
      return createElement(icons[icon as keyof typeof icons])
    }
  },
  pageTree: {
    transformers: [
      {
        file(node, filePath) {
          if (!filePath) return node
          const file = this.storage.read(filePath)
          if (file?.format !== "page") return node
          const {sidebarTitle} = file.data as {sidebarTitle?: string}
          if (!sidebarTitle) return node
          node.name = sidebarTitle
          return node
        },
      },
    ],
  },
})

export const llmSource = loader({
  baseUrl: "/llms.mdx/docs",
  url: slugs => `/llms.mdx/docs/${slugs.join("/")}.md`,
  source: docs.toFumadocsSource(),
})

export async function getSource() {
  return source
}

export async function getLlmSource() {
  return llmSource
}

type SourcePage = InferPageType<typeof source>
type LlmSourcePage = InferPageType<typeof llmSource>

function isVisibleSourcePage(page: SourcePage) {
  return (
    findPath(
      source.getPageTree(page.locale).children,
      node => node.type === "page" && node.url === page.url,
    ) !== null
  )
}

function isVisibleLlmSourcePage(page: LlmSourcePage) {
  return (
    findPath(
      llmSource.getPageTree(page.locale).children,
      node => node.type === "page" && node.url === page.url,
    ) !== null
  )
}

export function getVisiblePages(language?: string) {
  return source.getPages(language).filter(isVisibleSourcePage)
}

export function getVisibleLlmPages(language?: string) {
  return llmSource.getPages(language).filter(isVisibleLlmSourcePage)
}

export function generateVisibleParams() {
  return getVisiblePages().map(page => ({
    slug: page.slugs,
  }))
}

const generateVisibleSourceParams: typeof source.generateParams = (slug, lang) => {
  const slugName = slug ?? "slug"
  const langName = lang ?? "lang"

  return getVisiblePages().map(page => ({
    [slugName]: page.slugs,
    ...(page.locale ? {[langName]: page.locale} : {}),
  })) as ReturnType<typeof source.generateParams>
}

const generateVisibleLlmParams: typeof llmSource.generateParams = (slug, lang) => {
  const slugName = slug ?? "slug"
  const langName = lang ?? "lang"

  return getVisibleLlmPages().map(page => ({
    [slugName]: page.slugs,
    ...(page.locale ? {[langName]: page.locale} : {}),
  })) as ReturnType<typeof llmSource.generateParams>
}

export const visibleSource: typeof source = {
  ...source,
  getPages: getVisiblePages,
  getLanguages: () =>
    source.getLanguages().map(entry => ({
      ...entry,
      pages: getVisiblePages(entry.language),
    })),
  generateParams: generateVisibleSourceParams,
}

export const visibleLlmSource: typeof llmSource = {
  ...llmSource,
  getPages: getVisibleLlmPages,
  getLanguages: () =>
    llmSource.getLanguages().map(entry => ({
      ...entry,
      pages: getVisibleLlmPages(entry.language),
    })),
  generateParams: generateVisibleLlmParams,
}

export function getPageImage(page: InferPageType<typeof source>) {
  const segments = [...page.slugs, "image.png"]

  return {
    segments,
    url: `/og/docs/${segments.join("/")}`,
  }
}
