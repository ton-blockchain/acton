import { source } from '@/lib/source';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { baseOptions } from '@/lib/layout.shared';
import { NotFound as FumadocsNotFound } from '@/components/layouts/not-found';

export default function NotFound() {
  return (
    <DocsLayout
      tree={source.pageTree}
      githubUrl="https://github.com/ton-blockchain/acton"
      {...baseOptions()}
    >
      <FumadocsNotFound getSuggestions={async () => []} />
    </DocsLayout>
  );
}
