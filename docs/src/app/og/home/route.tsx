import { ImageResponse } from 'next/og';
import { generate, getImageResponseOptions } from '@/lib/mono';

export const revalidate = false;

export async function GET() {
  const options = await getImageResponseOptions();

  return new ImageResponse(
    await generate({
      title: 'Acton Documentation',
      description:
        'Blazingly fast toolkit, test runner, build system, formatter, and verifier for TON smart contract development.',
    }),
    options,
  );
}
