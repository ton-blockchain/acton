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
  images: { unoptimized: true },
  turbopack: {
    root: docsRoot,
  },
  basePath: isProd ? `/${repoName}` : "",
  assetPrefix: isProd ? `https://ton-blockchain.github.io/${repoName}/` : "",
};

export default withMDX(config);
