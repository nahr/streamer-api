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
  FormControl,
  InputLabel,
  Select,
  MenuItem,
} from '@mui/material'
import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import AddIcon from '@mui/icons-material/Add'
import RemoveIcon from '@mui/icons-material/Remove'
import StopIcon from '@mui/icons-material/Stop'
import PlayArrowIcon from '@mui/icons-material/PlayArrow'
import LiveTvIcon from '@mui/icons-material/LiveTv'
import { getCamera, parseCameraType, startRtmpStream } from '../api/cameras.js'
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
    playerOneRating: '',
    playerTwoRating: '',
    playerOneRatingType: 'Fargo',
    playerTwoRatingType: 'Fargo',
  })
  const [startError, setStartError] = useState('')
  const [scoreUpdating, setScoreUpdating] = useState(false)
  const [rtmpDialogOpen, setRtmpDialogOpen] = useState(false)
  const [rtmpUrl, setRtmpUrl] = useState('')
  const [rtmpError, setRtmpError] = useState('')
  const [rtmpStarting, setRtmpStarting] = useState(false)

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
    const { playerOneName, playerTwoName, playerOneRaceTo, playerTwoRaceTo, playerOneRating, playerTwoRating, playerOneRatingType, playerTwoRatingType } = startForm
    if (!playerOneName.trim() || !playerTwoName.trim()) {
      setStartError('Both player names are required')
      return
    }
    if (playerOneRaceTo < 1 || playerOneRaceTo > 21 || playerTwoRaceTo < 1 || playerTwoRaceTo > 21) {
      setStartError('Race to must be between 1 and 21 for each player')
      return
    }
    const p1Rating = playerOneRating.trim()
    const p2Rating = playerTwoRating.trim()
    const parseRating = (s) => {
      const n = parseInt(s, 10)
      return Number.isFinite(n) && n >= 0 ? n : null
    }
    const r1 = parseRating(p1Rating)
    const r2 = parseRating(p2Rating)
    setStartError('')
    try {
      await createMatch({
        player_one: {
          name: playerOneName.trim(),
          race_to: playerOneRaceTo,
          ...(r1 != null && { rating: { type: playerOneRatingType, value: r1 } }),
        },
        player_two: {
          name: playerTwoName.trim(),
          race_to: playerTwoRaceTo,
          ...(r2 != null && { rating: { type: playerTwoRatingType, value: r2 } }),
        },
        camera_name: camera.name,
      })
      setStartDialogOpen(false)
      setStartForm({ playerOneName: '', playerTwoName: '', playerOneRaceTo: 5, playerTwoRaceTo: 5, playerOneRating: '', playerTwoRating: '', playerOneRatingType: 'Fargo', playerTwoRatingType: 'Fargo' })
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

  const handleStartRtmp = async () => {
    if (!rtmpUrl.trim().startsWith('rtmp://')) {
      setRtmpError('Enter a valid RTMP URL (e.g. rtmp://a.rtmp.youtube.com/live2/xxxx)')
      return
    }
    setRtmpError('')
    setRtmpStarting(true)
    try {
      await startRtmpStream(camera.id, rtmpUrl.trim())
      setRtmpDialogOpen(false)
      setRtmpUrl('')
    } catch (err) {
      setRtmpError(err.message)
    } finally {
      setRtmpStarting(false)
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
          <Box sx={{ mt: 2, position: 'relative', display: 'inline-block' }}>
            <Button
              startIcon={<LiveTvIcon />}
              variant="outlined"
              onClick={() => setRtmpDialogOpen(true)}
              sx={{ mb: 2 }}
            >
              Go Live (RTMP)
            </Button>
            <img
              src={getStreamUrl(camera.id)}
              alt={`${camera.name} live stream`}
              style={{
                width: '100%',
                maxWidth: 640,
                borderRadius: 8,
                backgroundColor: '#000',
                display: 'block',
              }}
            />
            {activeMatch && !activeMatch.end_time && (
              <Box
                sx={{
                  position: 'absolute',
                  bottom: 0,
                  left: 0,
                  right: 0,
                  background: 'rgba(0,0,0,0.9)',
                  color: '#fff',
                  py: 1.25,
                  px: 2,
                  borderRadius: '0 0 8px 8px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: 2,
                }}
              >
                <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-start', minWidth: 0, flex: 1 }}>
                  <Typography variant="subtitle2" fontWeight={600} noWrap sx={{ maxWidth: '100%' }}>
                    {activeMatch.player_one.name}
                  </Typography>
                  {activeMatch.player_one.rating && (
                    <Typography variant="caption" color="rgba(255,255,255,0.8)">
                      {activeMatch.player_one.rating.type} {activeMatch.player_one.rating.value}
                    </Typography>
                  )}
                </Box>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, flexShrink: 0 }}>
                  <Box
                    sx={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      minWidth: 32,
                      height: 32,
                      borderRadius: '50%',
                      border: '2px solid #fff',
                    }}
                  >
                    <Typography variant="subtitle1" fontWeight={700}>
                      {activeMatch.player_one.games_won}
                    </Typography>
                  </Box>
                  <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center' }}>
                    <Typography variant="caption" color="rgba(255,255,255,0.8)">
                      race to
                    </Typography>
                    <Typography variant="caption" color="rgba(255,255,255,0.8)">
                      {activeMatch.player_one.race_to}/{activeMatch.player_two.race_to}
                    </Typography>
                  </Box>
                  <Box
                    sx={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      minWidth: 32,
                      height: 32,
                      borderRadius: '50%',
                      border: '2px solid #fff',
                    }}
                  >
                    <Typography variant="subtitle1" fontWeight={700}>
                      {activeMatch.player_two.games_won}
                    </Typography>
                  </Box>
                </Box>
                <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', minWidth: 0, flex: 1 }}>
                  <Typography variant="subtitle2" fontWeight={600} noWrap sx={{ maxWidth: '100%' }}>
                    {activeMatch.player_two.name}
                  </Typography>
                  {activeMatch.player_two.rating && (
                    <Typography variant="caption" color="rgba(255,255,255,0.8)">
                      {activeMatch.player_two.rating.type} {activeMatch.player_two.rating.value}
                    </Typography>
                  )}
                </Box>
              </Box>
            )}
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
                  {activeMatch.player_one.games_won >= activeMatch.player_one.race_to
                    ? `${activeMatch.player_one.name} won`
                    : activeMatch.player_two.games_won >= activeMatch.player_two.race_to
                      ? `${activeMatch.player_two.name} won`
                      : 'Match ended early'}
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

      <Dialog open={rtmpDialogOpen} onClose={() => setRtmpDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Go Live (RTMP)</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            Push the stream to YouTube Live, Facebook, or other RTMP destinations.
            The match overlay (player names, ratings, score) is burned into the stream.
          </Typography>
          <TextField
            label="RTMP URL"
            placeholder="e.g. rtmp://a.rtmp.youtube.com/live2/xxxx"
            value={rtmpUrl}
            onChange={(e) => setRtmpUrl(e.target.value)}
            fullWidth
            error={!!rtmpError}
            helperText={rtmpError}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRtmpDialogOpen(false)}>Cancel</Button>
          <Button variant="contained" onClick={handleStartRtmp} disabled={rtmpStarting}>
            {rtmpStarting ? 'Starting…' : 'Start stream'}
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={startDialogOpen} onClose={() => setStartDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Start pool match</DialogTitle>
        <DialogContent>
          <Stack direction="row" spacing={3} sx={{ mt: 1 }}>
            <Stack spacing={2} sx={{ flex: 1 }}>
              <Typography variant="subtitle2" color="text.secondary">Player 1</Typography>
              <TextField
                label="Name"
                value={startForm.playerOneName}
                onChange={(e) => setStartForm((f) => ({ ...f, playerOneName: e.target.value }))}
                fullWidth
                autoFocus
              />
              <FormControl fullWidth>
                <InputLabel>Rating type</InputLabel>
                <Select
                  value={startForm.playerOneRatingType}
                  label="Rating type"
                  onChange={(e) => setStartForm((f) => ({ ...f, playerOneRatingType: e.target.value }))}
                >
                  <MenuItem value="Fargo">Fargo</MenuItem>
                  <MenuItem value="Apa">APA</MenuItem>
                </Select>
              </FormControl>
              <TextField
                label="Rating (optional)"
                placeholder={startForm.playerOneRatingType === 'Fargo' ? 'e.g. 650' : 'e.g. 5'}
                type="number"
                value={startForm.playerOneRating}
                onChange={(e) => setStartForm((f) => ({ ...f, playerOneRating: e.target.value }))}
                inputProps={{ min: 0, max: startForm.playerOneRatingType === 'Apa' ? 9 : undefined }}
                fullWidth
              />
              <TextField
                label="Race to"
                type="number"
                value={startForm.playerOneRaceTo}
                onChange={(e) => setStartForm((f) => ({ ...f, playerOneRaceTo: parseInt(e.target.value, 10) || 5 }))}
                inputProps={{ min: 1, max: 21 }}
                fullWidth
              />
            </Stack>
            <Stack spacing={2} sx={{ flex: 1 }}>
              <Typography variant="subtitle2" color="text.secondary">Player 2</Typography>
              <TextField
                label="Name"
                value={startForm.playerTwoName}
                onChange={(e) => setStartForm((f) => ({ ...f, playerTwoName: e.target.value }))}
                fullWidth
              />
              <FormControl fullWidth>
                <InputLabel>Rating type</InputLabel>
                <Select
                  value={startForm.playerTwoRatingType}
                  label="Rating type"
                  onChange={(e) => setStartForm((f) => ({ ...f, playerTwoRatingType: e.target.value }))}
                >
                  <MenuItem value="Fargo">Fargo</MenuItem>
                  <MenuItem value="Apa">APA</MenuItem>
                </Select>
              </FormControl>
              <TextField
                label="Rating (optional)"
                placeholder={startForm.playerTwoRatingType === 'Fargo' ? 'e.g. 650' : 'e.g. 5'}
                type="number"
                value={startForm.playerTwoRating}
                onChange={(e) => setStartForm((f) => ({ ...f, playerTwoRating: e.target.value }))}
                inputProps={{ min: 0, max: startForm.playerTwoRatingType === 'Apa' ? 9 : undefined }}
                fullWidth
              />
              <TextField
                label="Race to"
                type="number"
                value={startForm.playerTwoRaceTo}
                onChange={(e) => setStartForm((f) => ({ ...f, playerTwoRaceTo: parseInt(e.target.value, 10) || 5 }))}
                inputProps={{ min: 1, max: 21 }}
                fullWidth
              />
            </Stack>
          </Stack>
          {startError && (
            <Typography color="error" variant="body2" sx={{ mt: 2 }}>
              {startError}
            </Typography>
          )}
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
