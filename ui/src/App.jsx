import { Routes, Route, Navigate } from 'react-router-dom'
import { Box, CircularProgress, Typography } from '@mui/material'

import { useApiInfo } from './apiInfoStore'
import { useAuth } from './authStore.jsx'
import { Registration } from './features/admin'
import { Layout } from './components/Layout'
import { Home } from './pages/Home'
import { Camera } from './features/cameras'
import { Admin } from './features/admin'

function App() {
  const { initialized, loading, retrying, refetch } = useApiInfo()
  const { isLoggedIn } = useAuth()

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
      </Box>
    )
  }

  if (!initialized) {
    return <Registration onSuccess={refetch} />
  }

  return (
    <Routes>
      <Route path="/" element={<Layout />}>
        <Route index element={<Home />} />
        <Route path="camera/:id" element={<Camera />} />
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
