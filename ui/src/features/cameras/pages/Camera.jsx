import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import {
  Box,
  Typography,
  Paper,
  CircularProgress,
  Button,
  Chip,
} from '@mui/material'
import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import { getCamera, parseCameraType } from '../api/cameras.js'

function formatCameraType(cameraType) {
  const parsed = parseCameraType(cameraType)
  if (parsed.type === 'rtsp') return { label: 'RTSP', detail: parsed.url || '(no url)' }
  if (parsed.type === 'usb') return { label: 'USB', detail: parsed.device || '(no device)' }
  return { label: 'Internal', detail: null }
}

export function Camera() {
  const { id } = useParams()
  const navigate = useNavigate()
  const [camera, setCamera] = useState(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!id) return
    let cancelled = false
    async function fetch() {
      setLoading(true)
      setError('')
      try {
        const data = await getCamera(id)
        if (!cancelled) setCamera(data)
      } catch (err) {
        if (!cancelled) setError(err.message)
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    fetch()
    return () => { cancelled = true }
  }, [id])

  if (loading) {
    return (
      <Box display="flex" justifyContent="center" py={4}>
        <CircularProgress />
      </Box>
    )
  }

  if (error || !camera) {
    return (
      <Box sx={{ p: 2 }}>
        <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
          Back
        </Button>
        <Typography color="error">{error || 'Camera not found'}</Typography>
      </Box>
    )
  }

  const { label, detail } = formatCameraType(camera.camera_type)

  return (
    <Box sx={{ p: 2 }}>
      <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
        Back to Home
      </Button>
      <Paper sx={{ p: 3 }}>
        <Box display="flex" alignItems="center" gap={2} sx={{ mb: 2 }}>
          <Typography variant="h4" component="h1">
            {camera.name}
          </Typography>
          <Chip label={label} size="small" />
        </Box>
        {detail && (
          <Typography color="text.secondary">
            {detail}
          </Typography>
        )}
      </Paper>
    </Box>
  )
}
