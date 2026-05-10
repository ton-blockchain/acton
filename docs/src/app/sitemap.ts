import type {MetadataRoute} from "next"
import {getVisiblePages} from "@/lib/source"

export const revalidate = false

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const baseUrl = "https://ton-blockchain.github.io/acton/"
  const url = (path: string): string => new URL(path.replace(/^\//, ""), baseUrl).toString()
  const excludedUrls = new Set(["/docs"])

  const docsPages = await Promise.all(
    getVisiblePages()
      .filter(page => !excludedUrls.has(page.url))
      .map(async page => {
        const {lastModified} = page.data

        const sitemapUrl: MetadataRoute.Sitemap[number] = {
          url: url(page.url),
          lastModified: lastModified ? new Date(lastModified) : undefined,
          changeFrequency: "weekly",
          priority: 0.7,
        }
        return sitemapUrl
      }),
  )

  return [
    {
      url: url("/"),
      changeFrequency: "monthly",
      priority: 1,
    },
    ...docsPages,
  ]
}
