import { NotFound as FumadocsNotFound } from '@/components/layouts/not-found';

export default function NotFound() {
  return <FumadocsNotFound getSuggestions={async () => []} />;
}
