import type {MetadataRoute} from "next"
import {baseUrl, docsEnvironment} from "@/lib/metadata"

export const revalidate = false

export default function robots(): MetadataRoute.Robots {
  if (docsEnvironment !== "production") {
    return {
      rules: {
        userAgent: "*",
        disallow: "/",
      },
    }
  }

  return {
    rules: {
      userAgent: "*",
      allow: "/",
    },
    sitemap: `${baseUrl}/sitemap.xml`,
  }
}
