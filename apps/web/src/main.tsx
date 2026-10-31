import React from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './style.css'

const root = document.getElementById('root')
if (!root) throw new Error('Myth Maker root element is missing')
const queries = new QueryClient({defaultOptions:{queries:{retry:1,refetchOnWindowFocus:true}}})
createRoot(root).render(
  <React.StrictMode>
    <QueryClientProvider client={queries}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
)
