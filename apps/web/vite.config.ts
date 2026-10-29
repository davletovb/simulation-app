import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  // Only explicitly public values may reach a browser bundle. VITE_* is NOT exposed.
  envPrefix: 'MYTH_PUBLIC_',
  plugins: [
    react(),
    {
      name: 'reject-browser-secret-environment-names',
      configResolved(config) {
        const unsafe = Object.keys(config.env).filter(name =>
          /^MYTH_PUBLIC_.*(KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL)/i.test(name),
        )
        if (unsafe.length) {
          throw new Error(`Refusing browser-exposed secret environment names: ${unsafe.join(', ')}`)
        }
      },
    },
  ],
  // In production the Rust service owns both assets and /api on one loopback origin.
  server: {
    host: '127.0.0.1',
    proxy: {
      '^/api(/|$)': { target: 'http://127.0.0.1:8787' },
    },
  },
  preview: { host: '127.0.0.1' },
})
