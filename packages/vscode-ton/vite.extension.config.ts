import fs from "node:fs"
import {builtinModules} from "node:module"
import path from "node:path"
import {fileURLToPath} from "node:url"

import {defineConfig, type Plugin} from "vite"

const rootDir = path.dirname(fileURLToPath(import.meta.url))
const nodeModules = new Set([...builtinModules, ...builtinModules.map(name => `node:${name}`)])

interface StaticAsset {
  readonly sourcePath: string
  readonly fileName: string
}

export default defineConfig({
  plugins: [extensionAssets()],
  resolve: {
    conditions: ["node"],
    mainFields: ["module", "main"],
  },
  build: {
    outDir: path.join(rootDir, "dist"),
    emptyOutDir: true,
    target: "node18",
    minify: "esbuild",
    lib: {
      entry: path.join(rootDir, "src/extension.ts"),
      formats: ["cjs"],
      fileName: () => "client.js",
    },
    rollupOptions: {
      external: id => id === "vscode" || nodeModules.has(id),
      output: {
        entryFileNames: "client.js",
        chunkFileNames: "chunks/[name]-[hash].js",
        exports: "named",
      },
    },
  },
})

function extensionAssets(): Plugin {
  return {
    name: "acton-vscode-static-assets",
    generateBundle() {
      for (const asset of staticAssets()) {
        this.emitFile({
          type: "asset",
          fileName: asset.fileName,
          source: fs.readFileSync(asset.sourcePath),
        })
      }
    },
  }
}

function staticAssets(): StaticAsset[] {
  return [
    ...directoryAssets("src/assets/icons", "icons"),
    ...directoryAssets("src/languages", "languages", fileName =>
      fileName.endsWith("-language-configuration.json"),
    ),
    ...directoryAssets("src/languages/syntaxes", "syntaxes", fileName =>
      fileName.endsWith(".tmLanguage.json"),
    ),
    {
      sourcePath: path.join(rootDir, "src/assets/logo.png"),
      fileName: "logo.png",
    },
    {
      sourcePath: path.join(rootDir, "syntaxes/tolk.tmLanguage.json"),
      fileName: "syntaxes/tolk.tmLanguage.json",
    },
  ]
}

function directoryAssets(
  sourceDirectory: string,
  outputDirectory: string,
  include: (fileName: string) => boolean = () => true,
): StaticAsset[] {
  const absoluteDirectory = path.join(rootDir, sourceDirectory)

  return fs
    .readdirSync(absoluteDirectory, {withFileTypes: true})
    .filter(entry => entry.isFile() && include(entry.name))
    .map(entry => ({
      sourcePath: path.join(absoluteDirectory, entry.name),
      fileName: path.posix.join(outputDirectory, entry.name),
    }))
}
