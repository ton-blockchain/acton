import type {Suggestion} from "@/components/layouts/not-found"
import {getVisiblePages} from "@/lib/source"

export async function getSuggestions(pathname: string): Promise<Suggestion[]> {
  const terms = new Set(
    pathname
      .toLowerCase()
      .split(/[^a-z0-9]+/)
      .filter(term => term.length > 1),
  )

  if (terms.size === 0) return []

  return getVisiblePages()
    .map(page => {
      const haystack = [page.url, page.path, page.data.title, page.data.description]
        .join(" ")
        .toLowerCase()

      let score = 0
      for (const term of terms) {
        if (haystack.includes(term)) score += 1
      }

      return {page, score}
    })
    .filter(({score}) => score > 0)
    .sort((a, b) => b.score - a.score || a.page.url.localeCompare(b.page.url))
    .slice(0, 5)
    .map(({page}) => ({
      id: page.url,
      href: page.url,
      title: page.data.title,
    }))
}
