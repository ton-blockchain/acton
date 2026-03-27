import defaultMdxComponents from 'fumadocs-ui/mdx';
import type { MDXComponents } from 'mdx/types';
import {
  CommandOption,
  CommandOptions,
  CommandOptionTitle,
} from '@/components/CommandOptions';

export function getMDXComponents(components?: MDXComponents): MDXComponents {
  return {
    ...defaultMdxComponents,
    CommandOption,
    CommandOptions,
    CommandOptionTitle,
    ...components,
  };
}
