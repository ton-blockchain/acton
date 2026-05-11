import {generateVisibleParams, getPageImage, source} from "@/lib/source"
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  PageLastUpdate,
  DocsTitle,
} from "fumadocs-ui/layouts/docs/page"
import {getMDXComponents} from "@/lib/mdx-components"
import type {Metadata} from "next"
import {createRelativeLink} from "fumadocs-ui/mdx"
import {LLMCopyButton, ViewOptions} from "@/components/page-actions"
import {getLLMText} from "@/lib/get-llm-text"
import {NotFound} from "@/components/layouts/not-found"
import {getSuggestions} from "./suggestions"

interface PageProps {
  params: Promise<{slug: string[]}>
}

const metadataBase = new URL("https://ton-blockchain.github.io/acton")

export default async function Page(props: PageProps) {
  const params = await props.params
  const page = source.getPage(params.slug)

  if (!page) {
    return <NotFound getSuggestions={async () => getSuggestions(params.slug.join(" "))} />
  }

  const {body: MDX, lastModified} = page.data

  const llmText = getLLMText(page)

  return (
    <DocsPage
      toc={page.data.toc}
      full={page.data.full}
      tableOfContent={{
        style: "clerk",
      }}
    >
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription className="mb-2">{page.data.description}</DocsDescription>
      <div className="flex flex-row flex-wrap gap-2 items-center border-b pb-6">
        <LLMCopyButton content={llmText} />
        <ViewOptions
          markdownUrl={`/llms.mdx${page.url}.md`}
          githubUrl={`https://github.com/ton-blockchain/acton/blob/master/docs/content/docs/${page.path}`}
        />
      </div>
      <DocsBody>
        <MDX
          components={getMDXComponents({
            // this allows you to link to other pages with relative file paths
            a: createRelativeLink(source, page),
          })}
        />
      </DocsBody>
      {lastModified && (
        <div className="mt-4 border-t pt-4">
          <PageLastUpdate date={lastModified} />
        </div>
      )}
    </DocsPage>
  )
}

export async function generateStaticParams() {
  return generateVisibleParams()
}

export async function generateMetadata(props: PageProps): Promise<Metadata> {
  const params = await props.params
  const page = source.getPage(params.slug)

  if (!page) {
    return {
      title: "Not Found",
      metadataBase,
    }
  }

  return {
    title: page.data.title,
    description: page.data.description,
    metadataBase,
    openGraph: {
      images: getPageImage(page).url,
    },
    twitter: {
      images: getPageImage(page).url,
    },
  }
}
