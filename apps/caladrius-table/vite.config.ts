import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Library build (`npm run build`): the package's own JS and CSS, with the TanStack code bundled
// and React left to the app (peer dependency). The demo has its own config in demo/.
export default defineConfig({
  plugins: [react()],
  build: {
    lib: { entry: 'src/index.ts', formats: ['es'], fileName: 'caladrius-table', cssFileName: 'table' },
    rollupOptions: { external: ['react', 'react-dom', 'react/jsx-runtime'] },
    outDir: 'dist',
    emptyOutDir: true,
    minify: true,
  },
});
