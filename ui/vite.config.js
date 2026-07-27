import path from 'path'
import { fileURLToPath } from 'url'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

const apiTarget = process.env.VITE_API_TARGET || 'http://localhost:8080'

export default defineConfig({
  envDir: __dirname,
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
  envPrefix: ['VITE_'],
  plugins: [react()],
  server: {
    host: '0.0.0.0',
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: false,
      },
    },
  },
})
