import {fileURLToPath} from "node:url"
import {createMDX} from "fumadocs-mdx/next"

const withMDX = createMDX()
const docsRoot = fileURLToPath(new URL(".", import.meta.url))

const isGitHubPagesBuild =
  process.env.GITHUB_ACTIONS === "true" || process.env.GITHUB_PAGES === "true"

const repoUrl = "https://ton-blockchain.github.io"
const repoName = "acton"

function resolveBaseUrl() {
  const publicUrl = process.env.NEXT_PUBLIC_SITE_URL
  if (publicUrl !== undefined && publicUrl !== "") {
    return publicUrl
  }

  if (isGitHubPagesBuild) {
    return `${repoUrl}/${repoName}`
  }

  return "http://localhost:3000"
}

function resolveBasePath() {
  if (isGitHubPagesBuild) {
    return `/${repoName}`
  }

  return undefined
}

function resolveAssetPrefix() {
  if (isGitHubPagesBuild) {
    return `${repoUrl}/${repoName}`
  }

  return undefined
}

const baseUrl = resolveBaseUrl()
const basePath = resolveBasePath()
const assetPrefix = resolveAssetPrefix()

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
  basePath: basePath,
  assetPrefix: assetPrefix,
}

export default withMDX(config)
