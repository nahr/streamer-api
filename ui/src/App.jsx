import { useState, useEffect } from 'react'
import { Routes, Route, Navigate } from 'react-router-dom'
import { Box, Button, CircularProgress, Typography } from '@mui/material'

import { useApiInfo } from './apiInfoStore.jsx'
import { Layout } from './components/Layout'
import { Home } from './pages/Home'
import { Camera, Match } from './features/cameras'
import { Admin } from './features/admin'

const LOADING_TIMEOUT_MS = 15000

function App() {
  const { loading, retrying, refetch } = useApiInfo()
  const [loadingTimedOut, setLoadingTimedOut] = useState(false)

  useEffect(() => {
    if (!loading) {
      setLoadingTimedOut(false)
      return
    }
    const t = setTimeout(() => setLoadingTimedOut(true), LOADING_TIMEOUT_MS)
    return () => clearTimeout(t)
  }, [loading])

  if (loading) {
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
              Ensure the API is running (port 8080).
            </Typography>
            <Button variant="outlined" size="small" onClick={() => refetch()}>
              Retry connection
            </Button>
          </Box>
        )}
      </Box>
    )
  }

  return (
    <Routes>
      <Route path="/" element={<Layout />}>
        <Route index element={<Home />} />
        <Route path="camera/:id" element={<Camera />} />
        <Route path="match/:id" element={<Match />} />
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
