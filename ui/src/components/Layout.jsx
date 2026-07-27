import { useNavigate, useLocation, Outlet } from 'react-router-dom'
import {
  AppBar,
  Box,
  Button,
  Container,
  Toolbar,
  Typography,
} from '@mui/material'
import AdminPanelSettingsIcon from '@mui/icons-material/AdminPanelSettings'
import { useApiInfo } from '../apiInfoStore.jsx'

export function Layout() {
  const navigate = useNavigate()
  const location = useLocation()
  const { locationName } = useApiInfo()

  return (
    <Box sx={{ flexGrow: 1 }}>
      <AppBar position="static">
        <Toolbar>
          {locationName ? (
            <Typography variant="h6" component="span" sx={{ mr: 2, fontWeight: 600 }}>
              {locationName}
            </Typography>
          ) : null}
          <Button
            color="inherit"
            onClick={() => navigate('/')}
            sx={{ fontWeight: location.pathname === '/' ? 700 : 400 }}
          >
            Home
          </Button>
          <Button
            color="inherit"
            onClick={() => navigate('/admin')}
            startIcon={<AdminPanelSettingsIcon />}
            sx={{
              ml: 1,
              fontWeight: location.pathname.startsWith('/admin') ? 700 : 400,
            }}
          >
            Admin
          </Button>
          <Box sx={{ flexGrow: 1 }} />
        </Toolbar>
      </AppBar>
      <Container maxWidth="lg" sx={{ py: 3 }}>
        <Outlet />
      </Container>
    </Box>
  )
}
