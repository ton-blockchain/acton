import {defineConfig, defineDocs} from "fumadocs-mdx/config"
import {transformerTwoslash} from "fumadocs-twoslash"
import {createFileSystemTypesCache} from "fumadocs-twoslash/cache-fs"
import {readFileSync} from "node:fs"
import type {LanguageRegistration, ShikiTransformer} from "shiki"
import tolkGrammarRaw from "./grammars/grammar-tolk.json"
import funcGrammarRaw from "./grammars/grammar-func.json"
import tasmGrammarRaw from "./grammars/grammar-tasm.json"
import tlbGrammarRaw from "./grammars/grammar-tlb.json"
import actonCliGrammarRaw from "./grammars/grammar-acton-cli.json"
import actonCliCheckGrammarRaw from "./grammars/grammar-acton-cli-check.json"
import actonCliWrapperGrammarRaw from "./grammars/grammar-acton-cli-wrapper.json"
import actonCliMutateGrammarRaw from "./grammars/grammar-acton-cli-mutate.json"
import actonCliTraceGrammarRaw from "./grammars/grammar-acton-cli-trace.json"
import actonTraceGrammarRaw from "./grammars/grammar-acton-trace.json"
import actonGasReportGrammarRaw from "./grammars/grammar-acton-gas-report.json"
import lastModified from "fumadocs-mdx/plugins/last-modified"
import {tolkTwoslasher} from "@/lib/tolk-twoslash"
import {pageSchema} from "fumadocs-core/source/schema"
import {remarkMdxFiles} from "fumadocs-core/mdx-plugins"
import {remarkMdxMermaid} from "fumadocs-core/mdx-plugins/remark-mdx-mermaid"
import {parseCodeBlockAttributes} from "fumadocs-core/mdx-plugins/codeblock-utils"
import {z} from "zod"
import {remarkDocsVariables} from "@/lib/remark-docs-variables"

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    schema: pageSchema.extend({
      description: z.string(),
      sidebarTitle: z.string().optional(),
    }),
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
})

// @ts-expect-error Tolk grammar type is wider than LanguageRegistration
const tolkGrammar: LanguageRegistration = {
  ...tolkGrammarRaw,
  name: "tolk",
}

const funcGrammar: LanguageRegistration = {
  ...funcGrammarRaw,
  name: "func",
}

const tasmGrammar: LanguageRegistration = {
  ...tasmGrammarRaw,
  name: "tasm",
}

const actonTraceGrammar: LanguageRegistration = {
  ...actonTraceGrammarRaw,
  name: "acton-trace",
}

// @ts-expect-error CLI grammar type is wider than LanguageRegistration
const actonCliGrammar: LanguageRegistration = {
  ...actonCliGrammarRaw,
  name: "acton-cli",
}

// @ts-expect-error CLI check grammar type is wider than LanguageRegistration
const actonCliCheckGrammar: LanguageRegistration = {
  ...actonCliCheckGrammarRaw,
  name: "acton-cli-check",
  embeddedLangs: ["acton-cli"],
}

const actonCliWrapperGrammar: LanguageRegistration = {
  ...actonCliWrapperGrammarRaw,
  name: "acton-cli-wrapper",
  embeddedLangs: ["acton-cli"],
}

// @ts-expect-error CLI mutate grammar type is wider than LanguageRegistration
const actonCliMutateGrammar: LanguageRegistration = {
  ...actonCliMutateGrammarRaw,
  name: "acton-cli-mutate",
  embeddedLangs: ["acton-cli"],
}

const actonCliTraceGrammar: LanguageRegistration = {
  ...actonCliTraceGrammarRaw,
  name: "acton-cli-trace",
  embeddedLangs: ["acton-cli", "acton-trace"],
}

const actonGasReportGrammar: LanguageRegistration = {
  ...actonGasReportGrammarRaw,
  name: "acton-gas-report",
}

// @ts-expect-error JSON grammar type is wider than LanguageRegistration
const tlbGrammar: LanguageRegistration = {
  ...tlbGrammarRaw,
  name: "tlb",
}

const actonTomlGrammar: LanguageRegistration = {
  name: "acton-toml",
  displayName: "Acton.toml",
  scopeName: "source.acton-toml",
  aliases: ["Acton.toml"],
  embeddedLangs: ["toml"],
  repository: {},
  patterns: [
    {
      include: "source.toml",
    },
  ],
}

const builtinLangs = [
  "bash",
  "fish",
  "json",
  "nushell",
  "powershell",
  "toml",
  "yaml",
  "typescript",
  "tsx",
] as const

const customLangs = [
  tolkGrammar,
  funcGrammar,
  tasmGrammar,
  actonCliGrammar,
  actonCliCheckGrammar,
  actonCliWrapperGrammar,
  actonCliMutateGrammar,
  actonCliTraceGrammar,
  actonTraceGrammar,
  actonGasReportGrammar,
  actonTomlGrammar,
  tlbGrammar,
] as const

const tolkFileIcon = readFileSync("public/logo-ton-gray.svg", "utf8")
const actonTomlIcon = readFileSync("public/logo-acton-file.svg", "utf8")
const terminalIcon = readFileSync("public/logo-terminal.svg", "utf8")

const transformerNoCopy: ShikiTransformer = {
  name: "acton:no-copy",
  pre(pre) {
    const raw = this.options?.meta?.__raw
    if (!raw) return pre

    const {attributes} = parseCodeBlockAttributes(raw, ["noCopy"])

    if ("noCopy" in attributes) {
      pre.properties.allowCopy = ""
    }

    return pre
  },
}

export default defineConfig({
  plugins: [lastModified()],
  mdxOptions: {
    rehypeCodeOptions: {
      lazy: false,
      icon: {
        extend: {
          "acton-cli": terminalIcon,
          "acton-cli-check": terminalIcon,
          "acton-cli-wrapper": terminalIcon,
          "acton-cli-mutate": terminalIcon,
          "acton-cli-trace": terminalIcon,
          "acton-gas-report": terminalIcon,
          "acton-toml": actonTomlIcon,
          tolk: tolkFileIcon,
        },
      },
      themes: {
        light: "one-light",
        dark: "one-dark-pro",
      },
      langs: [...builtinLangs, ...customLangs],
      transformers: [
        transformerNoCopy,
        transformerTwoslash({
          typesCache: createFileSystemTypesCache(),
          langs: ["tolk"],
          twoslasher: tolkTwoslasher,
        }),
      ],
    },
    remarkPlugins: [remarkDocsVariables, remarkMdxFiles, remarkMdxMermaid],
  },
})
