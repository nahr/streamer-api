import { useState, useEffect, useCallback } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import {
  Box,
  Button,
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
import { useAuth } from '../authStore.jsx'
import { useApiInfo } from '../apiInfoStore.jsx'
import { listCameras, formatCameraType, parseCameraType } from '../features/cameras/api/cameras.js'
import { listMatches } from '../features/cameras/api/poolMatches.js'
import { MatchDuration } from '../components/MatchDuration.jsx'
import { formatTime, getMatchWinner } from '../utils/format.js'

export function Home() {
  const navigate = useNavigate()
  const location = useLocation()
  const { isLoggedIn, isAdmin } = useAuth()
  const { camerasConfigured, loading: apiInfoLoading } = useApiInfo()
  const [cameras, setCameras] = useState([])
  const [matches, setMatches] = useState([])
  const [loading, setLoading] = useState(true)
  const [matchesLoading, setMatchesLoading] = useState(true)

  useEffect(() => {
    if (!isLoggedIn) {
      setCameras([])
      setLoading(false)
      return
    }
    setLoading(true)
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
  }, [isLoggedIn])

  // Direct admin to configure a camera when none exist
  useEffect(() => {
    if (isAdmin && !apiInfoLoading && !camerasConfigured) {
      navigate('/admin/camera-settings', { replace: true })
    }
  }, [isAdmin, apiInfoLoading, camerasConfigured, navigate])

  const fetchMatches = useCallback(async () => {
    setMatchesLoading(true)
    try {
      const data = await listMatches()
      setMatches([...data].sort((a, b) => b.start_time - a.start_time))
    } catch {
      setMatches([])
    } finally {
      setMatchesLoading(false)
    }
  }, [])

  useEffect(() => {
    if (location.pathname !== '/') return
    fetchMatches()
  }, [location.pathname, fetchMatches])

  useEffect(() => {
    if (location.pathname !== '/') return
    const onFocus = () => fetchMatches()
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [location.pathname, fetchMatches])

  const camerasInUse = new Set(
    matches.filter((m) => !m.end_time).map((m) => m.camera_id).filter(Boolean)
  )

  return (
    <Box sx={{ p: 2 }}>
      <Box display="flex" alignItems="center" justifyContent="space-between" sx={{ mb: 1 }}>
        <Typography variant="h6" component="h2">
          Matches
        </Typography>
        <Button size="small" onClick={fetchMatches} disabled={matchesLoading}>
          {matchesLoading ? 'Loading…' : 'Refresh'}
        </Button>
      </Box>
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
              const score = `${match.player_one.games_won} - ${match.player_two.games_won}`
              const winner = getMatchWinner(match)
              const secondary = (
                <>
                  {formatTime(match.start_time, 'short')}
                  {match.camera_name && <> · {match.camera_name}</>}
                  {' · '}
                  <MatchDuration match={match} />
                  {match.started_by && (
                    <> · Started by {match.started_by}</>
                  )}
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
                  onClick={() => navigate(`/match/${match.id}`)}
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

      {isLoggedIn && (
        <>
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
                {cameras.map((camera) => {
                  const inUse = camerasInUse.has(camera.id)
                  return (
                    <ListItemButton
                      key={camera.id}
                      onClick={() => !inUse && navigate(`/camera/${camera.id}`)}
                      disabled={inUse}
                    >
                      <VideocamIcon sx={{ mr: 2, color: 'text.secondary' }} />
                      <ListItemText
                        primary={
                          <>
                            {camera.name}
                            {inUse && (
                              <Chip
                                label="in use"
                                size="small"
                                component="span"
                                sx={{ ml: 1, verticalAlign: 'middle' }}
                              />
                            )}
                          </>
                        }
                        secondary={formatCameraType(camera.camera_type)}
                      />
                    </ListItemButton>
                  )
                })}
              </List>
            </Paper>
          )}
        </>
      )}
    </Box>
  )
}
