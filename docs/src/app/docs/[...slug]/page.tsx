import {generateVisibleParams, getPageImage, source} from "@/lib/source"
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
  PageLastUpdate,
} from "fumadocs-ui/layouts/docs/page"
import {getMDXComponents} from "@/lib/mdx-components"
import type {Metadata} from "next"
import {createRelativeLink} from "fumadocs-ui/mdx"
import {LLMCopyButton, ViewOptions} from "@/components/page-actions"
import {getLLMText} from "@/lib/get-llm-text"
import {NotFound} from "@/components/layouts/not-found"
import {getSuggestions} from "./suggestions"
import {baseUrl} from "@/lib/metadata"

interface PageProps {
  params: Promise<{slug: string[]}>
}

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
          markdownUrl={`${baseUrl}/llms.mdx${page.url}.md`}
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
      metadataBase: baseUrl,
    }
  }

  const image = getPageImage(page)

  return {
    title: page.data.title,
    description: page.data.description,
    metadataBase: baseUrl,
    alternates: {
      canonical: page.url,
    },
    openGraph: {
      title: page.data.title,
      description: page.data.description,
      url: page.url,
      type: "article",
      images: [
        {
          url: image.url,
          width: 1200,
          height: 630,
          alt: page.data.title,
        },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: page.data.title,
      description: page.data.description,
      images: [image.url],
    },
  }
}
