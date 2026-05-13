import {fileURLToPath} from "node:url"
import {createMDX} from "fumadocs-mdx/next"

const withMDX = createMDX()
const docsRoot = fileURLToPath(new URL(".", import.meta.url))

const isGitHubPagesBuild =
  process.env.GITHUB_ACTIONS === "true" || process.env.GITHUB_PAGES === "true"

const repoUrl = "https://ton-blockchain.github.io"
const repoName = "acton"

function resolveBaseUrl() {
  const publicUrl = process.env.NEXT_PUBLIC_SITE_URL;
  if (publicUrl !== undefined && publicUrl !== '') {
    return publicUrl;
  }

  if (isGitHubPagesBuild) {
    return `${repoUrl}/${repoName}`
  }

  return 'http://localhost:3000';
}

const baseUrl = resolveBaseUrl();

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  output: "export",
  env: {
    NEXT_PUBLIC_BASE_URL: baseUrl,
  },
  serverExternalPackages: ["typescript", "twoslash"],
  images: {unoptimized: true},
  turbopack: {
    root: docsRoot,
  },
  ...(isGitHubPagesBuild
    ? {
      basePath: `/${repoName}`,
      assetPrefix: `${repoUrl}/${repoName}/`,
    }
    : {}),
}

export default withMDX(config)
