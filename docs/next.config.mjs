import {fileURLToPath} from "node:url"
import {createMDX} from "fumadocs-mdx/next"

const withMDX = createMDX()
const docsRoot = fileURLToPath(new URL(".", import.meta.url))

const isGitHubPagesBuild =
  process.env.GITHUB_ACTIONS === "true" || process.env.GITHUB_PAGES === "true"

const repoUrl = "https://ton-blockchain.github.io"
const repoName = "acton"

const baseUrl = isGitHubPagesBuild ? `${repoUrl}/${repoName}` : "http://localhost:3000"

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
