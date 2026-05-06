import {defineConfig, defineDocs} from "fumadocs-mdx/config"
import {transformerTwoslash} from "fumadocs-twoslash"
import {createFileSystemTypesCache} from "fumadocs-twoslash/cache-fs"
import type {LanguageRegistration} from "shiki"
import tolkGrammarRaw from "./grammars/grammar-tolk.json"
import funcGrammarRaw from "./grammars/grammar-func.json"
import tasmGrammarRaw from "./grammars/grammar-tasm.json"
import tlbGrammarRaw from "./grammars/grammar-tlb.json"
import actonCliGrammarRaw from "./grammars/grammar-acton-cli.json"
import actonTraceGrammarRaw from "./grammars/grammar-acton-trace.json"
import lastModified from "fumadocs-mdx/plugins/last-modified"
import {tolkTwoslasher} from "@/lib/tolk-twoslash"
import {pageSchema} from "fumadocs-core/source/schema"
import {z} from "zod"

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    schema: pageSchema.extend({
      description: z.string(),
    }),
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
})

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

// @ts-expect-error JSON grammar type is wider than LanguageRegistration
const tlbGrammar: LanguageRegistration = {
  ...tlbGrammarRaw,
  name: "tlb",
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

export default defineConfig({
  plugins: [lastModified()],
  mdxOptions: {
    rehypeCodeOptions: {
      lazy: false,
      themes: {
        light: "one-light",
        dark: "one-dark-pro",
      },
      langs: [
        ...builtinLangs,
        tolkGrammar,
        funcGrammar,
        tasmGrammar,
        actonCliGrammar,
        actonTraceGrammar,
        tlbGrammar,
      ],
      transformers: [
        transformerTwoslash({
          typesCache: createFileSystemTypesCache(),
          langs: ["tolk"],
          twoslasher: tolkTwoslasher,
        }),
      ],
    },
  },
})
