# Acton Docs

This directory contains the documentation site for Acton. It is a static
[Next.js](https://nextjs.org/) application backed by
[Fumadocs](https://fumadocs.dev/) and MDX content, with a custom landing page,
docs navigation, local search, Open Graph image generation, and syntax
highlighting for Acton and TON-related languages.

## What lives here

- `content/docs/`: end-user documentation pages written in MDX.
- `content/docs/**/meta.json`: sidebar structure and section metadata.
- `src/app/`: Next.js routes for the landing page, docs pages, search endpoint,
  and generated OG images.
- `src/components/`: reusable UI for the landing page and docs experience.
- `src/lib/`: content loading, source mapping, and docs helpers.
- `grammars/`: custom Shiki grammars used for highlighted code blocks.
- `source.config.ts`: Fumadocs and MDX configuration for the docs collection.

## Local development

This workspace uses Bun.

```bash
bun install
bun run dev
```

Open `http://localhost:3000` for the landing page and
`http://localhost:3000/docs/welcome/` for the documentation entry point.

## Available scripts

- `bun run dev`: start the local development server.
- `bun run build`: produce the static production build.
- `bun run start`: serve the production build locally.
- `bun run lint`: run ESLint for the docs app.
- `bun run deploy`: build the static export and publish `out/` to GitHub Pages.

## Editing content

Most docs changes happen under `content/docs/`. When you add or move pages,
update the nearby `meta.json` so navigation stays correct. For richer content,
reuse the shared MDX components and docs UI instead of embedding one-off markup
directly into pages.
