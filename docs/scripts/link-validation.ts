import {type FileObject, type ScanResult, type ValidateConfig, scanURLs} from "next-validate-link"
import type {InferPageType} from "fumadocs-core/source"
import {source} from "@/lib/source"

export async function getLinkValidationInput(): Promise<{
  files: FileObject[]
  scanned: ScanResult
}> {
  const scanned = await scanURLs({
    preset: "next",
    populate: {
      "docs/[[...slug]]": source.getPages().map(page => {
        return {
          value: {
            slug: page.slugs,
          },
          hashes: getHeadings(page),
        }
      }),
    },
  })

  return {
    files: await getFiles(),
    scanned,
  }
}

export function createLinkValidationConfig(
  scanned: ScanResult,
  overrides: Partial<ValidateConfig> = {},
): ValidateConfig {
  return {
    scanned,
    markdown: {
      components: {
        Card: {attributes: ["href"]},
        Cards: {attributes: ["href"]},
        Link: {attributes: ["href"]},
        SourceCodeLink: {attributes: ["href"]},
        LandingVideo: {attributes: ["src"]},
      },
    },
    ...overrides,
  }
}

function getHeadings({data}: InferPageType<typeof source>): string[] {
  return data.toc.map(item => item.url.slice(1))
}

function getFiles() {
  const promises = source.getPages().map(
    async (page): Promise<FileObject> => ({
      path: page.absolutePath ?? page.path,
      content: await page.data.getText("raw"),
      url: page.url,
      data: page.data,
    }),
  )

  return Promise.all(promises)
}
