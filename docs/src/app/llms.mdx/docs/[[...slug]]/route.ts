import { getLLMText } from '@/lib/get-llm-text';
import { source } from '@/lib/source';
import { notFound } from 'next/navigation';

export const revalidate = false;

interface RouteProps {
  params: Promise<{ slug?: string[] }>;
}

function stripMarkdownExtension(slug?: string[]) {
  if (!slug || slug.length === 0) {
    return slug;
  }

  const last = slug.at(-1);

  if (!last?.endsWith('.md')) {
    return slug;
  }

  return [...slug.slice(0, -1), last.slice(0, -'.md'.length)];
}

function appendMarkdownExtension(slug: string[]) {
  if (slug.length === 0) {
    return slug;
  }

  const last = slug.at(-1);

  if (!last) {
    return slug;
  }

  return [...slug.slice(0, -1), `${last}.md`];
}

export async function GET(_request: Request, { params }: RouteProps) {
  const { slug } = await params;
  const page = source.getPage(stripMarkdownExtension(slug));

  if (!page) {
    notFound();
  }

  return new Response(await getLLMText(page), {
    headers: {
      'Content-Type': 'text/markdown; charset=utf-8',
    },
  });
}

export function generateStaticParams() {
  return source.generateParams().map(({ slug }) => ({
    slug: appendMarkdownExtension(slug),
  }));
}
