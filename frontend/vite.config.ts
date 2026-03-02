import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Proxy both HTTP and WebSocket to the backend during development.
// In production, nginx handles routing.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    proxy: {
      '/graphql': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        ws: true, // forward WebSocket upgrades for subscriptions
      },
    },
  },
});
