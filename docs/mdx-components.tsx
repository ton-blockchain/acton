import defaultMdxComponents from 'fumadocs-ui/mdx';
import * as Twoslash from 'fumadocs-twoslash/ui';
import type { MDXComponents } from 'mdx/types';
import {
  CommandOption,
  CommandOptionMeta,
  CommandOptions,
  CommandOptionTitle,
} from '@/components/CommandOptions';

export function getMDXComponents(components?: MDXComponents): MDXComponents {
  return {
    ...defaultMdxComponents,
    ...Twoslash,
    CommandOption,
    CommandOptionMeta,
    CommandOptions,
    CommandOptionTitle,
    ...components,
  };
}
