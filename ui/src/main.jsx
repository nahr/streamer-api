import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { ThemeProvider, createTheme, CssBaseline } from '@mui/material'
import { AuthProvider } from './authStore.jsx'
import { ApiInfoProvider } from './apiInfoStore.jsx'
import App from './App.jsx'

const theme = createTheme({
  palette: {
    mode: 'dark',
  },
})

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <BrowserRouter>
        <ApiInfoProvider>
          <AuthProvider>
            <App />
          </AuthProvider>
        </ApiInfoProvider>
      </BrowserRouter>
    </ThemeProvider>
  </StrictMode>,
)
