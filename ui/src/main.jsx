import { StrictMode, useState, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { Box, ThemeProvider, createTheme, CssBaseline, Typography, CircularProgress } from '@mui/material'
import { Auth0Provider } from '@auth0/auth0-react'
import { AuthProvider } from './authStore.jsx'
import { ApiInfoProvider } from './apiInfoStore.jsx'
import App from './App.jsx'

const theme = createTheme({
  palette: {
    mode: 'dark',
  },
})

// Build-time fallback (used when API not available, e.g. dev with API not running)
const buildDomain = import.meta.env.AUTH0_DOMAIN || ''
const buildClientId = import.meta.env.AUTH0_CLIENT_ID || ''
const buildAudience = import.meta.env.AUTH0_AUDIENCE || ''
const buildSkipAudience = import.meta.env.AUTH0_SKIP_AUDIENCE === 'true'
const buildConnection = import.meta.env.AUTH0_CONNECTION || undefined

function AppWithProviders() {
  const [config, setConfig] = useState(null)
  const [configError, setConfigError] = useState(null)

  useEffect(() => {
    fetch('/api/config')
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data?.auth0_domain && data?.auth0_client_id) {
          setConfig({
            domain: data.auth0_domain,
            clientId: data.auth0_client_id,
            audience: data.auth0_audience || '',
            skipAudience: data.auth0_skip_audience ?? false,
            connection: data.auth0_connection || undefined,
          })
        } else {
          // Fall back to build-time values (dev with table-tv.config in project)
          setConfig({
            domain: buildDomain,
            clientId: buildClientId,
            audience: buildAudience,
            skipAudience: buildSkipAudience,
            connection: buildConnection,
          })
        }
      })
      .catch(() => setConfigError('Could not load config from API'))
  }, [])

  if (config === null && !configError) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" minHeight="100vh">
        <CircularProgress />
      </Box>
    )
  }

  const domain = config?.domain || buildDomain
  const clientId = config?.clientId || buildClientId
  const audience = config?.audience ?? buildAudience
  const skipAudience = config?.skipAudience ?? buildSkipAudience
  const connection = config?.connection ?? buildConnection
  const auth0Configured = domain && clientId

  if (configError && !auth0Configured) {
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
          {configError}. Ensure the API is running and table-tv.config is configured.
        </Typography>
      </Box>
    )
  }

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
          Auth0 is not configured. Set [auth0] domain, client_id, and audience in table-tv.config.
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
        redirect_uri: window.location.origin,
        audience: skipAudience ? undefined : (audience || undefined),
        scope: 'openid profile email',
        ...(connection && { connection }),
      }}
    >
      <ApiInfoProvider>
        <AuthProvider skipAudience={skipAudience}>
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
