import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { ThemeProvider, createTheme, CssBaseline } from '@mui/material'
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
          <App />
        </ApiInfoProvider>
      </BrowserRouter>
    </ThemeProvider>
  </StrictMode>,
)
