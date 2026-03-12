import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  root: './src',
  plugins: [
    svelte(),
    tailwindcss()
  ],
  resolve: {
    alias: {
      $lib: path.resolve('./src/lib')
    }
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421
        }
      : undefined,
    watch: {
      usePolling: true,
      interval: 100,
      ignored: ['**/src-tauri/**', '**/node_modules/**']
    }
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    target:
      process.env.TAURI_ENV_PLATFORM == 'windows'
        ? 'chrome105'
        : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG
  }
});
