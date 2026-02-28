import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Box,
  Typography,
  List,
  ListItemButton,
  ListItemText,
  CircularProgress,
  Paper,
  Chip,
} from '@mui/material'
import VideocamIcon from '@mui/icons-material/Videocam'
import SportsEsportsIcon from '@mui/icons-material/SportsEsports'
import { listCameras, parseCameraType } from '../features/cameras/api/cameras.js'
import { listMatches } from '../features/cameras/api/poolMatches.js'

function formatCameraType(cameraType) {
  const parsed = parseCameraType(cameraType)
  if (parsed.type === 'rtsp') return `RTSP: ${parsed.url || '(no url)'}`
  if (parsed.type === 'usb') return `USB: ${parsed.device || '(no device)'}`
  return 'Internal'
}

function formatTime(ms) {
  const d = new Date(ms)
  return d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
}

function formatDuration(ms) {
  const totalSeconds = Math.floor(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) {
    return `${hours}h ${minutes}m ${seconds}s`
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

function getMatchWinner(match) {
  if (!match.end_time) return null
  if (match.player_one.games_won >= match.player_one.race_to) return match.player_one.name
  if (match.player_two.games_won >= match.player_two.race_to) return match.player_two.name
  return null
}

function MatchDuration({ match }) {
  const [now, setNow] = useState(Date.now())
  useEffect(() => {
    if (match.end_time) return
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [match.end_time])
  const endMs = match.end_time ?? now
  const durationMs = endMs - match.start_time
  return <>{formatDuration(durationMs)}</>
}

export function Home() {
  const navigate = useNavigate()
  const [cameras, setCameras] = useState([])
  const [matches, setMatches] = useState([])
  const [loading, setLoading] = useState(true)
  const [matchesLoading, setMatchesLoading] = useState(true)

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

  useEffect(() => {
    let cancelled = false
    async function fetch() {
      try {
        const data = await listMatches()
        if (!cancelled) {
          setMatches([...data].sort((a, b) => b.start_time - a.start_time))
        }
      } catch {
        if (!cancelled) setMatches([])
      } finally {
        if (!cancelled) setMatchesLoading(false)
      }
    }
    fetch()
    return () => { cancelled = true }
  }, [])

  const cameraByName = Object.fromEntries(cameras.map((c) => [c.name, c]))

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Home
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 2 }}>
        Welcome to Table TV.
      </Typography>

      <Typography variant="h6" component="h2" gutterBottom>
        Matches
      </Typography>
      {matchesLoading ? (
        <Box display="flex" justifyContent="center" py={2}>
          <CircularProgress size={24} />
        </Box>
      ) : matches.length === 0 ? (
        <Typography color="text.secondary" sx={{ mb: 3 }}>
          No matches yet. Start a match from a camera view.
        </Typography>
      ) : (
        <Paper variant="outlined" sx={{ mb: 3 }}>
          <List disablePadding>
            {matches.map((match) => {
              const camera = cameraByName[match.camera_name]
              const score = `${match.player_one.games_won} - ${match.player_two.games_won}`
              const winner = getMatchWinner(match)
              const secondary = (
                <>
                  {formatTime(match.start_time)} · <MatchDuration match={match} />
                  {match.end_time && (
                    <>
                      {' '}
                      <Chip
                        label={winner ? `${winner} won` : 'Ended early'}
                        size="small"
                        component="span"
                        sx={{ verticalAlign: 'middle' }}
                      />
                    </>
                  )}
                </>
              )
              return (
                <ListItemButton
                  key={match.id}
                  onClick={() => camera && navigate(`/camera/${camera.id}`)}
                  disabled={!camera}
                >
                  <SportsEsportsIcon sx={{ mr: 2, color: 'text.secondary' }} />
                  <ListItemText
                    primary={
                      <>
                        {match.player_one.name} vs {match.player_two.name}
                        <Typography component="span" variant="body1" sx={{ ml: 1, fontWeight: 600 }}>
                          {score}
                        </Typography>
                      </>
                    }
                    secondary={secondary}
                  />
                </ListItemButton>
              )
            })}
          </List>
        </Paper>
      )}

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
