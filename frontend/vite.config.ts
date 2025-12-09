import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      'trading-bot-wasm': path.resolve(__dirname, 'src/wasm'),
    },
  },
  optimizeDeps: {
    exclude: ['trading-bot-wasm'],
  },
})
