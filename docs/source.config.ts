import { defineDocs, defineConfig } from 'fumadocs-mdx/config';
import type { RehypeCodeOptions } from 'fumadocs-core/mdx-plugins';
import type { LanguageRegistration } from 'shiki';
import tolkGrammarRaw from './grammars/grammar-tolk.json';
import funcGrammarRaw from './grammars/grammar-func.json';
import tasmGrammarRaw from './grammars/grammar-tasm.json';
import tlbGrammarRaw from './grammars/grammar-tlb.json';

export const docs = defineDocs({
    dir: 'content/docs',
    docs: {
        postprocess: {
            includeProcessedMarkdown: true,
        },
    },
});

// @ts-ignore
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

// @ts-ignore
const tlbGrammar: LanguageRegistration = {
    ...tlbGrammarRaw,
    name: 'tlb',
};

export default defineConfig({
    mdxOptions: {
        rehypeCodeOptions: {
            theme: 'one-dark-pro',
            themes: {
                light: 'one-light',
                dark: 'one-dark-pro',
            },
            langs: [
                tolkGrammar,
                funcGrammar,
                tasmGrammar,
                tlbGrammar,
            ],
        } as RehypeCodeOptions,
    },
});
