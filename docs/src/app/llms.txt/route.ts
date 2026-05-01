import { llms } from 'fumadocs-core/source';
import { getLlmSource } from '@/lib/source';

export const revalidate = false;

export async function GET() {
  const docs = await getLlmSource();
  const body = llms(docs).index();

  return new Response(body, {
    headers: {
      'Content-Type': 'text/plain; charset=utf-8',
    },
  });
}
