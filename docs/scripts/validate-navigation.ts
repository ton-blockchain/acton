import {source} from "@/lib/source"
import {findPath} from "fumadocs-core/page-tree"
import type {ValidateResult} from "next-validate-link"
import {printErrors} from "next-validate-link"
import fs from "node:fs"
import path from "node:path"

type MetaFile = {
  pages?: string[]
}

const metaCache = new Map<string, MetaFile | null>()

function validateDocsNavigation() {
  const results: ValidateResult[] = []

  for (const page of source.getPages()) {
    const pageTree = source.getPageTree(page.locale)
    const pagePath = findPath(
      pageTree.children,
      node => node.type === "page" && node.url === page.url,
    )
    if (pagePath !== null) {
      continue
    }
    if (isExcludedFromNavigation(page.path)) {
      continue
    }

    const metaPath = path.posix.join("content/docs", path.posix.dirname(page.path), "meta.json")

    let result = results.find(item => item.file === metaPath)
    if (!result) {
      result = {
        file: metaPath,
        detected: [],
        errors: [],
      }
      results.push(result)
    }

    const entry = path.basename(page.path, ".mdx")
    const message = `not listed in navigation, add "${entry}" to pages`

    result.errors.push({
      url: page.path,
      line: 1,
      column: 1,
      reason: new Error(message),
    })
  }

  printErrors(results, true)
}

function isExcludedFromNavigation(pagePath: string): boolean {
  const pageWithoutExtension = pagePath.replace(/\.mdx$/, "")
  const segments = pagePath.split("/")

  for (let index = 0; index < segments.length; index++) {
    const dir = segments.slice(0, index).join("/")
    const relativePath = segments
      .slice(index)
      .join("/")
      .replace(/\.mdx$/, "")
    const firstSegment = relativePath.split("/", 1)[0]
    const meta = readMeta(dir)

    if (
      meta?.pages?.some(item => {
        if (!item.startsWith("!")) {
          return false
        }

        const excluded = item.slice(1)
        return (
          excluded === firstSegment ||
          excluded === relativePath ||
          path.posix.join(dir, excluded) === pageWithoutExtension
        )
      })
    ) {
      return true
    }
  }

  return false
}

function readMeta(dir: string): MetaFile | null {
  const metaPath = path.join("content/docs", dir, "meta.json")
  if (metaCache.has(metaPath)) {
    return metaCache.get(metaPath) ?? null
  }

  if (!fs.existsSync(metaPath)) {
    metaCache.set(metaPath, null)
    return null
  }

  const meta = JSON.parse(fs.readFileSync(metaPath, "utf8")) as MetaFile
  metaCache.set(metaPath, meta)
  return meta
}

void validateDocsNavigation()
