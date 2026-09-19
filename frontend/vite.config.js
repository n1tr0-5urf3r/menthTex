import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import frontendPackage from './package.json' with { type: 'json' }
import process from 'node:process'

function normalizeBasePath(basePath) {
  if (!basePath || basePath === '/') {
    return '/'
  }

  const withLeadingSlash = basePath.startsWith('/') ? basePath : `/${basePath}`
  return withLeadingSlash.endsWith('/') ? withLeadingSlash : `${withLeadingSlash}/`
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const configuredBasePath = process.env.VITE_BASE_PATH || env.VITE_BASE_PATH || '/'

  return {
    base: normalizeBasePath(configuredBasePath),
    plugins: [react()],
    define: {
      'import.meta.env.VITE_APP_VERSION': JSON.stringify(frontendPackage.version),
    },
    server: {
      proxy: {
        '/api': 'http://127.0.0.1:3000',
      },
    },
  }
})
