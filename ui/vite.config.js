import path from 'path'
import { fileURLToPath } from 'url'
import fs from 'fs'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

// API port when running UI dev server (npm run dev). API runs at 8080 locally, 80 in Docker.
const apiTarget = process.env.VITE_API_TARGET || 'http://localhost:8080'

// https://vite.dev/config/
// Parse .env file directly - shell env vars override loadEnv(), which can send wrong client_id to Auth0
function parseEnvFile(dir) {
  const env = {}
  const file = path.join(dir, '.env')
  if (!fs.existsSync(file)) return env
  const content = fs.readFileSync(file, 'utf8')
  for (const line of content.split('\n')) {
    const trimmed = line.split('#')[0].trim()
    const m = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)
    if (m) env[m[1]] = m[2].replace(/^["']|["']$/g, '').trim()
  }
  return env
}

export default defineConfig(() => {
  const envDir = path.resolve(__dirname, '..')
  const env = parseEnvFile(envDir)
  const define = {}
  if (env.AUTH0_DOMAIN) define['import.meta.env.AUTH0_DOMAIN'] = JSON.stringify(env.AUTH0_DOMAIN)
  if (env.AUTH0_CLIENT_ID) define['import.meta.env.AUTH0_CLIENT_ID'] = JSON.stringify(env.AUTH0_CLIENT_ID)
  if (env.AUTH0_AUDIENCE) define['import.meta.env.AUTH0_AUDIENCE'] = JSON.stringify(env.AUTH0_AUDIENCE)
  if (env.AUTH0_REDIRECT_URI) define['import.meta.env.AUTH0_REDIRECT_URI'] = JSON.stringify(env.AUTH0_REDIRECT_URI)
  if (env.AUTH0_SKIP_AUDIENCE) define['import.meta.env.AUTH0_SKIP_AUDIENCE'] = JSON.stringify(env.AUTH0_SKIP_AUDIENCE)
  if (env.AUTH0_CONNECTION) define['import.meta.env.AUTH0_CONNECTION'] = JSON.stringify(env.AUTH0_CONNECTION)

  return {
  envDir,
  // Don't expose AUTH0_* from process.env - we use define from .env file only (shell overrides break Auth0)
  envPrefix: ['VITE_'],
  define,
  plugins: [react()],
  server: {
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: true,
      },
    },
  },
  }
})
