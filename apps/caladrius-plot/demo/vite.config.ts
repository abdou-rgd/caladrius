import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The demo reads crates/caladrius-ui/src/theme.json, two levels above the package.
export default defineConfig({
  plugins: [react()],
  server: { fs: { allow: ['../../..'] } },
});
