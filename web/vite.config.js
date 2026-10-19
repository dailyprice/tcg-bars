import { defineConfig } from 'vite';

// In dev, Vite serves the frontend and forwards API calls to the Rust server.
export default defineConfig({
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:8080',
    },
  },
});
