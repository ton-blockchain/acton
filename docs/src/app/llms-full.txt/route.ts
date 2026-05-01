import { getSource } from '@/lib/source';
import { getLLMText } from '@/lib/get-llm-text';

// cached forever
export const revalidate = false;

export async function GET() {
    const docs = await getSource();
    const scan = docs.getPages().map(getLLMText);
    const scanned = await Promise.all(scan);

    return new Response(scanned.join('\n\n'));
}
