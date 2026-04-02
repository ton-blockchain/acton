import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  root: 'app',
  plugins: [react()],
  build: {
    emptyOutDir: true,
    outDir: '../dist',
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return undefined;
          }

          if (id.includes('/react/') || id.includes('/react-dom/')) {
            return 'react';
          }

          if (id.includes('/@ton/ton/') || id.includes('/@ton/core/')) {
            return 'ton-sdk';
          }

          if (id.includes('/@tonconnect/')) {
            return 'tonconnect';
          }

          if (id.includes('/@ton/walletkit/')) {
            return 'walletkit';
          }

          if (
            id.includes('/@ton/appkit') ||
            id.includes('/@radix-ui/') ||
            id.includes('/react-remove-scroll/') ||
            id.includes('/focus-lock/') ||
            id.includes('/use-sidecar/') ||
            id.includes('/use-callback-ref/') ||
            id.includes('/aria-hidden/') ||
            id.includes('/tslib/')
          ) {
            return 'appkit-ui';
          }

          return undefined;
        },
      },
    },
  },
  server: {
    fs: {
      allow: ['..'],
    },
    host: '0.0.0.0',
    port: 5173,
  },
});
