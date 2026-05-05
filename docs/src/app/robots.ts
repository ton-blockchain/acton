import type {MetadataRoute} from "next"

export const revalidate = false

export default function robots(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: "*",
      allow: "/",
    },
    sitemap: "https://ton-blockchain.github.io/acton/sitemap.xml",
  }
}
