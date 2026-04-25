import { createMdxPlugin } from 'fumadocs-mdx/bun';
import { postInstall } from 'fumadocs-mdx/next';

Bun.plugin(createMdxPlugin());
await postInstall({ configPath: 'source.config.ts' });
