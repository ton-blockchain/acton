import './polyfills';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { TonConnectUIProvider, THEME } from '@tonconnect/ui-react';

const manifestUrl =
  'https://ton-blockchain.github.io/acton/tonconnect-manifest.json';

const savedTheme = localStorage.getItem('jm-theme');
const initialTheme = savedTheme === 'light' ? THEME.LIGHT : THEME.DARK;

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

document.documentElement.setAttribute(
  'data-theme',
  savedTheme === 'light' ? 'light' : 'dark',
);

async function bootstrap() {
  const { App } = await import('./App');

  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <TonConnectUIProvider
        manifestUrl={manifestUrl}
        uiPreferences={{
          theme: initialTheme,
          colorsSet: { [THEME.DARK]: darkColors, [THEME.LIGHT]: lightColors },
        }}
        analytics={{ mode: 'off' }}
      >
        <App />
      </TonConnectUIProvider>
    </StrictMode>,
  );
}

void bootstrap();
