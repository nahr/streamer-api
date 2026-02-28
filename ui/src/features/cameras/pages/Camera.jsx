import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import {
  Box,
  Typography,
  Paper,
  CircularProgress,
  Button,
  Chip,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  IconButton,
  Stack,
} from '@mui/material'
import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import AddIcon from '@mui/icons-material/Add'
import RemoveIcon from '@mui/icons-material/Remove'
import StopIcon from '@mui/icons-material/Stop'
import PlayArrowIcon from '@mui/icons-material/PlayArrow'
import { getCamera, parseCameraType } from '../api/cameras.js'
import { getActiveMatch, createMatch, updateScore, endMatch } from '../api/poolMatches.js'

function formatCameraType(cameraType) {
  const parsed = parseCameraType(cameraType)
  if (parsed.type === 'rtsp') return { label: 'RTSP', detail: parsed.url || '(no url)' }
  if (parsed.type === 'usb') return { label: 'USB', detail: parsed.device || '(no device)' }
  return { label: 'Internal', detail: null }
}

function getStreamUrl(id) {
  return `/api/cameras/${id}/stream`
}

export function Camera() {
  const { id } = useParams()
  const navigate = useNavigate()
  const [camera, setCamera] = useState(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [activeMatch, setActiveMatch] = useState(null)
  const [matchLoading, setMatchLoading] = useState(false)
  const [startDialogOpen, setStartDialogOpen] = useState(false)
  const [startForm, setStartForm] = useState({
    playerOneName: '',
    playerTwoName: '',
    playerOneRaceTo: 5,
    playerTwoRaceTo: 5,
  })
  const [startError, setStartError] = useState('')
  const [scoreUpdating, setScoreUpdating] = useState(false)

  const fetchActiveMatch = useCallback(async () => {
    if (!camera?.name) return
    setMatchLoading(true)
    try {
      const m = await getActiveMatch(camera.name)
      setActiveMatch(m)
    } catch {
      setActiveMatch(null)
    } finally {
      setMatchLoading(false)
    }
  }, [camera?.name])

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

  useEffect(() => {
    if (camera?.name) fetchActiveMatch()
  }, [camera?.name, fetchActiveMatch])

  const handleStartMatch = async () => {
    const { playerOneName, playerTwoName, playerOneRaceTo, playerTwoRaceTo } = startForm
    if (!playerOneName.trim() || !playerTwoName.trim()) {
      setStartError('Both player names are required')
      return
    }
    if (playerOneRaceTo < 1 || playerOneRaceTo > 21 || playerTwoRaceTo < 1 || playerTwoRaceTo > 21) {
      setStartError('Race to must be between 1 and 21 for each player')
      return
    }
    setStartError('')
    try {
      await createMatch({
        player_one: { name: playerOneName.trim(), race_to: playerOneRaceTo },
        player_two: { name: playerTwoName.trim(), race_to: playerTwoRaceTo },
        camera_name: camera.name,
      })
      setStartDialogOpen(false)
      setStartForm({ playerOneName: '', playerTwoName: '', playerOneRaceTo: 5, playerTwoRaceTo: 5 })
      await fetchActiveMatch()
    } catch (err) {
      setStartError(err.message)
    }
  }

  const handleScoreChange = async (player, delta) => {
    if (!activeMatch || scoreUpdating) return
    const p = player === 1 ? activeMatch.player_one : activeMatch.player_two
    const next = Math.max(0, Math.min(p.race_to, p.games_won + delta))
    if (next === p.games_won) return
    setScoreUpdating(true)
    setActiveMatch((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        player_one:
          player === 1 ? { ...prev.player_one, games_won: next } : prev.player_one,
        player_two:
          player === 2 ? { ...prev.player_two, games_won: next } : prev.player_two,
      }
    })
    try {
      const updated = await updateScore(activeMatch.id, player, next)
      setActiveMatch(updated)
      if (updated.end_time) {
        await fetchActiveMatch()
      }
    } catch {
      setActiveMatch(activeMatch)
    } finally {
      setScoreUpdating(false)
    }
  }

  const handleEndMatch = async () => {
    if (!activeMatch || scoreUpdating) return
    setScoreUpdating(true)
    try {
      await endMatch(activeMatch.id)
      await fetchActiveMatch()
    } finally {
      setScoreUpdating(false)
    }
  }

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
  const parsed = parseCameraType(camera.camera_type)
  const isInternal = parsed.type === 'internal'

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
        {isInternal && (
          <Box sx={{ mt: 2 }}>
            <img
              src={getStreamUrl(camera.id)}
              alt={`${camera.name} live stream`}
              style={{
                width: '100%',
                maxWidth: 640,
                borderRadius: 8,
                backgroundColor: '#000',
              }}
            />
          </Box>
        )}

        <Box sx={{ mt: 3, pt: 2, borderTop: 1, borderColor: 'divider' }}>
          <Typography variant="h6" sx={{ mb: 2 }}>
            Pool Match
          </Typography>
          {matchLoading ? (
            <CircularProgress size={24} />
          ) : activeMatch ? (
            <Stack spacing={2}>
              <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} alignItems="center" flexWrap="wrap">
                <Box display="flex" alignItems="center" gap={0.5}>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(1, -1)}
                    disabled={scoreUpdating || activeMatch.player_one.games_won === 0}
                    aria-label="Decrease player 1 score"
                  >
                    <RemoveIcon />
                  </IconButton>
                  <Typography variant="h5" component="span" sx={{ minWidth: 40, textAlign: 'center' }}>
                    {activeMatch.player_one.games_won}
                  </Typography>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(1, 1)}
                    disabled={scoreUpdating || activeMatch.player_one.games_won >= activeMatch.player_one.race_to}
                    aria-label="Increase player 1 score"
                  >
                    <AddIcon />
                  </IconButton>
                  <Typography sx={{ ml: 1 }}>{activeMatch.player_one.name}</Typography>
                  <Typography color="text.secondary" variant="body2" sx={{ ml: 0.5 }}>
                    (race to {activeMatch.player_one.race_to})
                  </Typography>
                </Box>
                <Typography color="text.secondary">vs</Typography>
                <Box display="flex" alignItems="center" gap={0.5}>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(2, -1)}
                    disabled={scoreUpdating || activeMatch.player_two.games_won === 0}
                    aria-label="Decrease player 2 score"
                  >
                    <RemoveIcon />
                  </IconButton>
                  <Typography variant="h5" component="span" sx={{ minWidth: 40, textAlign: 'center' }}>
                    {activeMatch.player_two.games_won}
                  </Typography>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(2, 1)}
                    disabled={scoreUpdating || activeMatch.player_two.games_won >= activeMatch.player_two.race_to}
                    aria-label="Increase player 2 score"
                  >
                    <AddIcon />
                  </IconButton>
                  <Typography sx={{ ml: 1 }}>{activeMatch.player_two.name}</Typography>
                  <Typography color="text.secondary" variant="body2" sx={{ ml: 0.5 }}>
                    (race to {activeMatch.player_two.race_to})
                  </Typography>
                </Box>
              </Stack>
              {!activeMatch.end_time && (
                <Button
                  startIcon={<StopIcon />}
                  variant="outlined"
                  color="secondary"
                  onClick={handleEndMatch}
                  disabled={scoreUpdating}
                >
                  End match early
                </Button>
              )}
              {activeMatch.end_time && (
                <Typography color="text.secondary" variant="body2">
                  Match ended
                </Typography>
              )}
            </Stack>
          ) : (
            <Button
              startIcon={<PlayArrowIcon />}
              variant="contained"
              onClick={() => setStartDialogOpen(true)}
            >
              Start match
            </Button>
          )}
        </Box>
      </Paper>

      <Dialog open={startDialogOpen} onClose={() => setStartDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Start pool match</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ mt: 1 }}>
            <TextField
              label="Player 1 name"
              value={startForm.playerOneName}
              onChange={(e) => setStartForm((f) => ({ ...f, playerOneName: e.target.value }))}
              fullWidth
              autoFocus
            />
            <TextField
              label="Player 2 name"
              value={startForm.playerTwoName}
              onChange={(e) => setStartForm((f) => ({ ...f, playerTwoName: e.target.value }))}
              fullWidth
            />
            <TextField
              label="Player 1 race to"
              type="number"
              value={startForm.playerOneRaceTo}
              onChange={(e) => setStartForm((f) => ({ ...f, playerOneRaceTo: parseInt(e.target.value, 10) || 5 }))}
              inputProps={{ min: 1, max: 21 }}
              fullWidth
            />
            <TextField
              label="Player 2 race to"
              type="number"
              value={startForm.playerTwoRaceTo}
              onChange={(e) => setStartForm((f) => ({ ...f, playerTwoRaceTo: parseInt(e.target.value, 10) || 5 }))}
              inputProps={{ min: 1, max: 21 }}
              fullWidth
            />
            {startError && (
              <Typography color="error" variant="body2">
                {startError}
              </Typography>
            )}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setStartDialogOpen(false)}>Cancel</Button>
          <Button variant="contained" onClick={handleStartMatch}>
            Start
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  )
}
