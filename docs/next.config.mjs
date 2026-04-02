import { fileURLToPath } from "node:url";
import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

const isProd = process.env.NODE_ENV === "production";
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
};

export default withMDX(config);
