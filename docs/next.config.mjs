import { fileURLToPath } from "node:url";
import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

const isProd = process.env.NODE_ENV === "production" ||
  process.env.GITHUB_ACTIONS === "true" ||
  process.env.GITHUB_PAGES === "true";
const repoName = "acton";
const docsRoot = fileURLToPath(new URL(".", import.meta.url));

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
  turbopack: {
    root: docsRoot,
  },
  basePath: isProd ? `/${repoName}` : "",
  assetPrefix: isProd ? `https://ton-blockchain.github.io/${repoName}/` : "",
  redirects: () => [
    // index.mdx → overview.mdx
    {
      source: '/docs/testing',
      destination: '/docs/testing/overview',
      permanent: true,
    },
    {
      source: '/docs/linting',
      destination: '/docs/linting/overview',
      permanent: true,
    },
    {
      source: '/docs/localnet',
      destination: '/docs/localnet/overview',
      permanent: true,
    },
    {
      source: '/docs/tolk_standard_library',
      destination: '/docs/tolk_standard_library/overview',
      permanent: true,
    },
    {
      source: '/docs/standard_library',
      destination: '/docs/standard_library/overview',
      permanent: true,
    },
    {
      source: '/docs/commands',
      destination: '/docs/commands/overview',
      permanent: true,
    },
    {
      source: '/docs/testing/test-ui',
      destination: '/docs/testing/test-ui/overview',
      permanent: true,
    },
    {
      source: '/docs/testing/mutation-testing',
      destination: '/docs/testing/mutation-testing/overview',
      permanent: true,
    },
    {
      source: '/docs/scripting',
      destination: '/docs/scripting/overview',
      permanent: true,
    },
    {
      source: '/docs/building',
      destination: '/docs/building/overview',
      permanent: true,
    },
    {
      source: '/docs/tutorial',
      destination: '/docs/tutorial/overview',
      permanent: true,
    },
    {
      source: '/docs/ide-support',
      destination: '/docs/ide-support/vscode',
      permanent: true,
    },
    {
      source: '/docs/rules',
      destination: '/docs/rules/overview',
      permanent: true,
    },
  ],
};

export default withMDX(config);
