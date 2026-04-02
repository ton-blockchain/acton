import type { PropsWithChildren } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import { AppKitProvider } from '@ton/appkit-react';
import '@ton/appkit-react/styles.css';

import { appKit, queryClient } from '../lib/ton';

export function AppProviders({ children }: PropsWithChildren) {
  return (
    <QueryClientProvider client={queryClient}>
      <AppKitProvider appKit={appKit}>{children}</AppKitProvider>
    </QueryClientProvider>
  );
}
