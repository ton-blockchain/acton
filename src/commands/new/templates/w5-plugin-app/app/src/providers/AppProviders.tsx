import { useEffect, useState, type PropsWithChildren } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import { TonConnectUIProvider, THEME } from '@tonconnect/ui-react';

import { queryClient } from '../lib/ton';

const manifestUrl =
  'https://ton-connect.github.io/demo-dapp-with-react-ui/tonconnect-manifest.json';

const darkColors = {
  background: {
    primary: '#19191B',
    secondary: '#19191B',
    segment: '#19191B',
    tint: '#19191B',
    qr: '#FFFFFF',
  },
  connectButton: { background: '#0098EA', foreground: '#FFFFFF' },
};

const lightColors = {
  background: {
    primary: '#FFFFFF',
    secondary: '#F0F1F3',
    segment: '#FFFFFF',
    tint: '#F0F1F3',
    qr: '#F0F1F3',
  },
  connectButton: { background: '#0098EA', foreground: '#FFFFFF' },
};

function readInitialTheme() {
  if (typeof window === 'undefined') return THEME.DARK;
  return localStorage.getItem('simple-extension-theme') === 'light'
    ? THEME.LIGHT
    : THEME.DARK;
}

export function AppProviders({ children }: PropsWithChildren) {
  const [initialTheme] = useState(readInitialTheme);

  useEffect(() => {
    const saved = localStorage.getItem('simple-extension-theme');
    document.documentElement.setAttribute(
      'data-theme',
      saved === 'light' ? 'light' : 'dark',
    );
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <TonConnectUIProvider
        manifestUrl={manifestUrl}
        uiPreferences={{
          theme: initialTheme,
          colorsSet: { [THEME.DARK]: darkColors, [THEME.LIGHT]: lightColors },
        }}
      >
        {children}
      </TonConnectUIProvider>
    </QueryClientProvider>
  );
}
