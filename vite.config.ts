import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    allowedHosts: ['1420-it0xozhe5flu3zmkihu7v-5f0e70b7.us4.manus.computer'],
  },
  build: { target: ['es2021', 'safari15'] },
});
