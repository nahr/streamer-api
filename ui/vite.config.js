import path from 'path'
import { fileURLToPath } from 'url'
import fs from 'fs'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

// API port when running UI dev server (npm run dev).
const apiTarget = process.env.VITE_API_TARGET || 'http://localhost:8080'

// https://vite.dev/config/
// Load Auth0 config from table-tv.config (same as API). Search order: /etc/table-tv/table-tv.config, ../table-tv.config, ../api/table-tv.config.
function loadAuth0FromConfig() {
  const root = path.resolve(__dirname, '..')
  const candidates = [
    '/etc/table-tv/table-tv.config',
    path.join(root, 'table-tv.config'),
    path.join(root, 'api', 'table-tv.config'),
  ]
  for (const file of candidates) {
    if (!fs.existsSync(file)) continue
    const content = fs.readFileSync(file, 'utf8')
    const auth0 = {}
    let inAuth0 = false
    for (const line of content.split('\n')) {
      const trimmed = line.trim()
      if (trimmed === '[auth0]') {
        inAuth0 = true
        continue
      }
      if (inAuth0) {
        if (trimmed.startsWith('[')) break // next section
        const m = trimmed.match(/^(\w+)\s*=\s*(?:"([^"]*)"|'([^']*)'|(\S+))$/)
        if (m) {
          const val = m[2] ?? m[3] ?? m[4] ?? ''
          if (m[1] === 'domain') auth0.AUTH0_DOMAIN = val
          else if (m[1] === 'client_id') auth0.AUTH0_CLIENT_ID = val
          else if (m[1] === 'audience') auth0.AUTH0_AUDIENCE = val
          else if (m[1] === 'skip_audience') auth0.AUTH0_SKIP_AUDIENCE = val === 'true' ? 'true' : ''
          else if (m[1] === 'connection') auth0.AUTH0_CONNECTION = val
        }
      }
    }
    if (Object.keys(auth0).length > 0) return auth0
  }
  return {}
}

export default defineConfig(() => {
  const auth0 = loadAuth0FromConfig()
  const define = {}
  if (auth0.AUTH0_DOMAIN) define['import.meta.env.AUTH0_DOMAIN'] = JSON.stringify(auth0.AUTH0_DOMAIN)
  if (auth0.AUTH0_CLIENT_ID) define['import.meta.env.AUTH0_CLIENT_ID'] = JSON.stringify(auth0.AUTH0_CLIENT_ID)
  if (auth0.AUTH0_AUDIENCE) define['import.meta.env.AUTH0_AUDIENCE'] = JSON.stringify(auth0.AUTH0_AUDIENCE)
  if (auth0.AUTH0_SKIP_AUDIENCE) define['import.meta.env.AUTH0_SKIP_AUDIENCE'] = JSON.stringify(auth0.AUTH0_SKIP_AUDIENCE)
  if (auth0.AUTH0_CONNECTION) define['import.meta.env.AUTH0_CONNECTION'] = JSON.stringify(auth0.AUTH0_CONNECTION)

  return {
  envDir: __dirname, // Use ui/ only - do not load .env from project root
  resolve: {
    dedupe: ['react', 'react-dom'],
  },
  // Don't expose AUTH0_* from process.env - we use define from table-tv.config only (shell overrides break Auth0)
  envPrefix: ['VITE_'],
  define,
  plugins: [react()],
  server: {
    host: "0.0.0.0",
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: false, // Preserve Host so API derives correct base_url for OAuth callbacks (e.g. localhost:5173, not :8080)
      },
    },
  },
  }
})
