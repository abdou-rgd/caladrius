import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The demo reads theme.json (crates/) and the recorded answer (test/fixtures): allow the repository.
export default defineConfig({
  plugins: [react()],
  server: { fs: { allow: ['../../..'] } },
});
