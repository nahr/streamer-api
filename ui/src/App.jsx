import { useState, useEffect } from 'react'
import { Routes, Route, Navigate, useLocation } from 'react-router-dom'
import { Box, Button, CircularProgress, Typography } from '@mui/material'

import { useApiInfo } from './apiInfoStore.jsx'
import { useAuth0 } from '@auth0/auth0-react'
import { useAuth } from './authStore.jsx'
import { Layout } from './components/Layout'
import { Home } from './pages/Home'
import { Camera, Match } from './features/cameras'
import { Admin } from './features/admin'
import { FacebookCallback } from './pages/FacebookCallback'

/** Wraps a route that requires authentication. Redirects to login if not logged in. */
function RequireAuth({ children }) {
  const location = useLocation()
  const { loginWithRedirect } = useAuth0()
  const { isLoggedIn, loading } = useAuth()

  useEffect(() => {
    if (!loading && !isLoggedIn) {
      loginWithRedirect({ appState: { returnTo: location.pathname } })
    }
  }, [loading, isLoggedIn, loginWithRedirect, location.pathname])

  if (loading || !isLoggedIn) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" minHeight={200}>
        <CircularProgress />
      </Box>
    )
  }
  return children
}

const LOADING_TIMEOUT_MS = 15000

function App() {
  const location = useLocation()
  const { initialized, loading, retrying, refetch } = useApiInfo()
  const { isLoading: auth0Loading, error: auth0Error } = useAuth0()
  const [loadingTimedOut, setLoadingTimedOut] = useState(false)

  useEffect(() => {
    if (!loading && !auth0Loading) {
      setLoadingTimedOut(false)
      return
    }
    const t = setTimeout(() => setLoadingTimedOut(true), LOADING_TIMEOUT_MS)
    return () => clearTimeout(t)
  }, [loading, auth0Loading])

  if (location.pathname === '/facebook/callback') {
    return <FacebookCallback />
  }

  if (auth0Error) {
    return (
      <Box
        display="flex"
        flexDirection="column"
        alignItems="center"
        justifyContent="center"
        gap={2}
        minHeight="100vh"
      >
        <Typography color="error">Auth0 error: {auth0Error.message}</Typography>
        <Button variant="outlined" onClick={() => window.location.replace(window.location.pathname)}>
          Clear and retry
        </Button>
      </Box>
    )
  }

  if (loading || auth0Loading) {
    return (
      <Box
        display="flex"
        flexDirection="column"
        alignItems="center"
        justifyContent="center"
        gap={2}
        minHeight="100vh"
      >
        <CircularProgress />
        <Typography color="text.secondary">
          {retrying ? 'Connecting... Retrying every 5 seconds.' : 'Loading...'}
        </Typography>
        {loadingTimedOut && (
          <Box display="flex" flexDirection="column" alignItems="center" gap={1} mt={2}>
            <Typography color="text.secondary" variant="body2">
              Taking longer than expected?
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Ensure the API is running (port 8080). If you just returned from login, try clearing the URL.
            </Typography>
            <Button
              variant="outlined"
              size="small"
              onClick={() => window.location.replace(window.location.pathname)}
            >
              Clear URL and refresh
            </Button>
            <Button variant="outlined" size="small" onClick={() => refetch()}>
              Retry connection
            </Button>
          </Box>
        )}
      </Box>
    )
  }

  if (!initialized) {
    return (
      <Box
        display="flex"
        flexDirection="column"
        alignItems="center"
        justifyContent="center"
        gap={2}
        minHeight="100vh"
      >
        <Typography color="text.secondary">
          Auth0 is not configured. Set AUTH0_DOMAIN, AUTH0_CLIENT_ID, and AUTH0_AUDIENCE in your .env file.
        </Typography>
      </Box>
    )
  }

  return (
    <Routes>
      <Route path="/" element={<Layout />}>
        <Route index element={<Home />} />
        <Route path="camera/:id" element={<RequireAuth><Camera /></RequireAuth>} />
        <Route path="match/:id" element={<RequireAuth><Match /></RequireAuth>} />
        <Route path="admin" element={<Admin />} />
        <Route path="admin/server-settings" element={<Admin />} />
        <Route path="admin/camera-settings" element={<Admin />} />
        <Route path="admin/matches" element={<Admin />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}

export default App
