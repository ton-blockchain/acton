import {source} from "@/lib/source"
import {findPath} from "fumadocs-core/page-tree"
import type {ValidateResult} from "next-validate-link"
import {printErrors} from "next-validate-link"
import path from "node:path"

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

    const metaPath = path.join("content/docs", path.dirname(page.path), "meta.json")

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

void validateDocsNavigation()
