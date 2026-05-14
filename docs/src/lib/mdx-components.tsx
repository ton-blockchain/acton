import defaultMdxComponents from "fumadocs-ui/mdx"
import {Cards, Card} from "fumadocs-ui/components/card"
import * as Twoslash from "fumadocs-twoslash/ui"
import type {MDXComponents} from "mdx/types"
import {
  CommandOption,
  CommandOptionMeta,
  CommandOptions,
  CommandOptionTitle,
} from "@/components/CommandOptions"
import {Callout} from "@/components/Callout"
import {File, Files, Folder} from "@/components/Files"
import {ImageZoom} from "@/components/image-zoom"
import {Mermaid} from "@/components/Mermaid"

export function getMDXComponents(components?: MDXComponents): MDXComponents {
  return {
    ...defaultMdxComponents,
    ...Twoslash,
    CommandOption,
    CommandOptionMeta,
    CommandOptions,
    CommandOptionTitle,
    Cards,
    Card,
    File,
    Folder,
    Files,
    Mermaid,
    // See: https://www.fumadocs.dev/docs/ui/components/image-zoom
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    img: props => <ImageZoom {...(props as any)} />,
    // Slightly modified Callout component:
    // https://github.com/fuma-nama/fumadocs/blob/db93ebdf6d73424001f1602509eb7f845f990f02/packages/base-ui/src/components/callout.tsx
    Callout,
    ...components,
  }
}
