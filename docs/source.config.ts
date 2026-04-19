import { defineConfig, defineDocs } from 'fumadocs-mdx/config';
import type { RehypeCodeOptions } from 'fumadocs-core/mdx-plugins';
import type { LanguageRegistration } from 'shiki';
import tolkGrammarRaw from './grammars/grammar-tolk.json';
import funcGrammarRaw from './grammars/grammar-func.json';
import tasmGrammarRaw from './grammars/grammar-tasm.json';
import tlbGrammarRaw from './grammars/grammar-tlb.json';
import actonCliGrammarRaw from './grammars/grammar-acton-cli.json';
import actonTraceGrammarRaw from './grammars/grammar-acton-trace.json';
import lastModified from 'fumadocs-mdx/plugins/last-modified';

export const docs = defineDocs({
    dir: 'content/docs',
    docs: {
        postprocess: {
            includeProcessedMarkdown: true,
        },
    },
});

const tolkGrammar: LanguageRegistration = {
    ...tolkGrammarRaw,
    name: 'tolk',
};

const funcGrammar: LanguageRegistration = {
    ...funcGrammarRaw,
    name: 'func',
};

const tasmGrammar: LanguageRegistration = {
    ...tasmGrammarRaw,
    name: 'tasm',
};

const actonTraceGrammar: LanguageRegistration = {
    ...actonTraceGrammarRaw,
    name: 'acton-trace',
};

// @ts-expect-error CLI grammar type is wider than LanguageRegistration
const actonCliGrammar: LanguageRegistration = {
    ...actonCliGrammarRaw,
    name: 'acton-cli',
};

// @ts-expect-error JSON grammar type is wider than LanguageRegistration
const tlbGrammar: LanguageRegistration = {
    ...tlbGrammarRaw,
    name: 'tlb',
};

export default defineConfig({
    plugins: [
      lastModified(),
    ],
    mdxOptions: {
        rehypeCodeOptions: {
            themes: {
                light: 'one-light',
                dark: 'one-dark-pro',
            },
            langs: [
                tolkGrammar,
                funcGrammar,
                tasmGrammar,
                actonCliGrammar,
                actonTraceGrammar,
                tlbGrammar,
            ],
        } as RehypeCodeOptions,
    },
});
