import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { Box, ThemeProvider, createTheme, CssBaseline, Typography } from '@mui/material'
import { Auth0Provider } from '@auth0/auth0-react'
import { AuthProvider } from './authStore.jsx'
import { ApiInfoProvider } from './apiInfoStore.jsx'
import App from './App.jsx'

const theme = createTheme({
  palette: {
    mode: 'dark',
  },
})

const domain = import.meta.env.AUTH0_DOMAIN || ''
const clientId = import.meta.env.AUTH0_CLIENT_ID || ''
const audience = import.meta.env.AUTH0_AUDIENCE || ''

// Optional: override redirect (e.g. http://127.0.0.1:5173 if localhost causes 403)
const redirectUri = import.meta.env.AUTH0_REDIRECT_URI || window.location.origin
// If true, skip audience (uses ID token instead of access token). Use when audience causes 403.
const skipAudience = import.meta.env.AUTH0_SKIP_AUDIENCE === 'true'
const connection = import.meta.env.AUTH0_CONNECTION || undefined

const auth0Configured = domain && clientId

function AppWithProviders() {
  if (!auth0Configured) {
    return (
      <Box
        display="flex"
        flexDirection="column"
        alignItems="center"
        justifyContent="center"
        minHeight="100vh"
        p={2}
      >
        <Typography color="text.secondary" textAlign="center">
          Auth0 is not configured. Set AUTH0_DOMAIN, AUTH0_CLIENT_ID, and AUTH0_AUDIENCE in your .env file.
        </Typography>
      </Box>
    )
  }

  return (
    <Auth0Provider
      domain={domain}
      clientId={clientId}
      cacheLocation="localstorage"
      authorizationParams={{
        redirect_uri: redirectUri,
        audience: skipAudience ? undefined : (audience || undefined),
        scope: 'openid profile email',
        ...(connection && { connection }),
      }}
    >
      <ApiInfoProvider>
        <AuthProvider>
          <App />
        </AuthProvider>
      </ApiInfoProvider>
    </Auth0Provider>
  )
}

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <BrowserRouter>
        <AppWithProviders />
      </BrowserRouter>
    </ThemeProvider>
  </StrictMode>,
)
