import { useNavigate, useLocation } from 'react-router-dom'
import { Box, Tab, Tabs } from '@mui/material'
import { useAuth } from '../../../authStore.jsx'
import { Login } from '../components/login'
import { ServerSettings } from './ServerSettings'
import { CameraSettings } from './CameraSettings'
import { MatchesSettings } from './MatchesSettings'

const TAB_PATHS = ['/admin/server-settings', '/admin/camera-settings', '/admin/matches']

export function Admin() {
  const navigate = useNavigate()
  const location = useLocation()
  const { isLoggedIn, login } = useAuth()

  if (!isLoggedIn) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" minHeight={400} p={2}>
        <Login onSuccess={login} />
      </Box>
    )
  }

  const tabIndex = TAB_PATHS.includes(location.pathname)
    ? TAB_PATHS.indexOf(location.pathname)
    : 0

  const handleTabChange = (_, newIndex) => {
    navigate(TAB_PATHS[newIndex])
  }

  return (
    <Box>
      <Tabs value={tabIndex} onChange={handleTabChange} sx={{ mb: 2 }}>
        <Tab label="Server Settings" />
        <Tab label="Camera Settings" />
        <Tab label="Matches" />
      </Tabs>
      {tabIndex === 0 && <ServerSettings />}
      {tabIndex === 1 && <CameraSettings />}
      {tabIndex === 2 && <MatchesSettings />}
    </Box>
  )
}
