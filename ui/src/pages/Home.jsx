import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Box, Typography, List, ListItemButton, ListItemText, CircularProgress, Paper } from '@mui/material'
import VideocamIcon from '@mui/icons-material/Videocam'
import { listCameras, parseCameraType } from '../features/cameras/api/cameras.js'

function formatCameraType(cameraType) {
  const parsed = parseCameraType(cameraType)
  if (parsed.type === 'rtsp') return `RTSP: ${parsed.url || '(no url)'}`
  if (parsed.type === 'usb') return `USB: ${parsed.device || '(no device)'}`
  return 'Internal'
}

export function Home() {
  const navigate = useNavigate()
  const [cameras, setCameras] = useState([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    async function fetch() {
      try {
        const data = await listCameras()
        if (!cancelled) setCameras(data)
      } catch {
        if (!cancelled) setCameras([])
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    fetch()
    return () => { cancelled = true }
  }, [])

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Home
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 2 }}>
        Welcome to Table TV.
      </Typography>

      <Typography variant="h6" component="h2" gutterBottom>
        Cameras
      </Typography>
      {loading ? (
        <Box display="flex" justifyContent="center" py={4}>
          <CircularProgress />
        </Box>
      ) : cameras.length === 0 ? (
        <Typography color="text.secondary">
          No cameras configured. Add cameras in Admin → Camera Settings.
        </Typography>
      ) : (
        <Paper variant="outlined">
          <List disablePadding>
            {cameras.map((camera) => (
              <ListItemButton
                key={camera.id}
                onClick={() => navigate(`/camera/${camera.id}`)}
              >
                <VideocamIcon sx={{ mr: 2, color: 'text.secondary' }} />
                <ListItemText
                  primary={camera.name}
                  secondary={formatCameraType(camera.camera_type)}
                />
              </ListItemButton>
            ))}
          </List>
        </Paper>
      )}
    </Box>
  )
}
