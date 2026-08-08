import './polyfills';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import './styles.css';

async function bootstrap() {
  const [{ default: App }, { AppProviders }] = await Promise.all([
    import('./App'),
    import('./providers/AppProviders'),
  ]);

  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <AppProviders>
        <App />
      </AppProviders>
    </StrictMode>,
  );
}

void bootstrap();
