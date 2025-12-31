import { createMDX } from "fumadocs-mdx/next";
const withMDX = createMDX();

const isProd = process.env.NODE_ENV === "production";
const repoName = "acton";  // Change to your repo name

/** @type {import('next').NextConfig} */
const config = {
    reactStrictMode: true,
    output: "export",              // Replaces `next export`
    trailingSlash: true,           // /docs → /docs/
    images: { unoptimized: true }, // Required for static export

    basePath: isProd ? `/${repoName}` : "",
    assetPrefix: isProd
        ? `https://i582.github.io/${repoName}/`  // Full URL = no 404
        : "",
};

export default withMDX(config);
