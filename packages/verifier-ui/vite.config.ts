import {readFile} from "node:fs/promises"
import {resolve} from "node:path"
import process from "node:process"

import react from "@vitejs/plugin-react"
import {defineConfig} from "vite"

import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

const backendTarget = process.env.VITE_BACKEND_PROXY_TARGET || "http://127.0.0.1:3000"
const contractHtml = resolve(import.meta.dirname, "contract.html")
const statisticsHtml = resolve(import.meta.dirname, "statistics.html")
const verifiedHtml = resolve(import.meta.dirname, "verified.html")

function routeHtmlPath(pathname: string, production = false) {
  if (pathname === "/statistics" || pathname === "/statistics/") {
    return production ? resolve(import.meta.dirname, "dist/statistics.html") : statisticsHtml
  }
  if (pathname === "/verified" || pathname === "/verified/") {
    return production ? resolve(import.meta.dirname, "dist/verified.html") : verifiedHtml
  }

  return production ? resolve(import.meta.dirname, "dist/contract.html") : contractHtml
}

function contractRouteFallback() {
  return {
    name: "contract-route-fallback",
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        const url = request.url ? new URL(request.url, "http://localhost") : undefined
        const pathname = url?.pathname ?? "/"
        const acceptsHtml = request.headers.accept?.includes("text/html") ?? false
        const isAsset = pathname.split("/").at(-1)?.includes(".") ?? false

        if (
          request.method === "GET" &&
          acceptsHtml &&
          pathname !== "/" &&
          !pathname.startsWith("/api/") &&
          !isAsset
        ) {
          const html = await readFile(routeHtmlPath(pathname), "utf8")
          const transformedHtml = await server.transformIndexHtml(pathname, html)
          response.statusCode = 200
          response.setHeader("Content-Type", "text/html")
          response.end(transformedHtml)
          return
        }

        next()
      })
    },
    configurePreviewServer(server) {
      server.middlewares.use(async (request, response, next) => {
        const url = request.url ? new URL(request.url, "http://localhost") : undefined
        const pathname = url?.pathname ?? "/"
        const acceptsHtml = request.headers.accept?.includes("text/html") ?? false
        const isAsset = pathname.split("/").at(-1)?.includes(".") ?? false

        if (
          request.method === "GET" &&
          acceptsHtml &&
          pathname !== "/" &&
          !pathname.startsWith("/api/") &&
          !isAsset
        ) {
          const html = await readFile(routeHtmlPath(pathname, true), "utf8")
          response.statusCode = 200
          response.setHeader("Content-Type", "text/html")
          response.end(html)
          return
        }

        next()
      })
    },
  }
}

export default defineConfig({
  plugins: [themeBootstrap({storageKey: "ton-verifier-theme"}), react(), contractRouteFallback()],
  resolve: {
    dedupe: ["react", "react-dom"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: resolve(import.meta.dirname, "index.html"),
        contract: resolve(import.meta.dirname, "contract.html"),
        statistics: resolve(import.meta.dirname, "statistics.html"),
        verified: resolve(import.meta.dirname, "verified.html"),
      },
    },
  },
  server: {
    port: 3007,
    proxy: {
      "^/api(?:/|$)": {
        target: backendTarget,
        changeOrigin: true,
      },
    },
  },
})
