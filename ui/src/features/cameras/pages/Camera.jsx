import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import {
  Alert,
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
import EditIcon from '@mui/icons-material/Edit'
import StopIcon from '@mui/icons-material/Stop'
import PlayArrowIcon from '@mui/icons-material/PlayArrow'
import LiveTvIcon from '@mui/icons-material/LiveTv'
import HistoryIcon from '@mui/icons-material/History'
import { getCamera, getRtmpStreamStatus, formatCameraType, startRtmpStream, stopRtmpStream } from '../api/cameras.js'
import { getActiveMatch, createMatch, updateScore, endMatch, listMatches } from '../api/poolMatches.js'
import { formatTime, formatDuration, formatMatchTitle, isRecordingAvailable, formatRecordingFilename } from '../../../utils/format.js'
import { useApiInfo } from '../../../apiInfoStore.jsx'
import { LiveTimestamp } from '../../../components/LiveTimestamp.jsx'
import { StreamPreview } from '../components/StreamPreview.jsx'
import { MatchScoreControls } from '../components/MatchScoreControls.jsx'
import { RecordingTimelineBar } from '../components/RecordingTimelineBar.jsx'
import { DownloadRecordingButton } from '../components/DownloadRecordingButton.jsx'
import { MatchHistory } from '../components/MatchHistory.jsx'

export function Camera() {
  const { id } = useParams()
  const navigate = useNavigate()
  const { locationName, recordDeleteAfter } = useApiInfo()
  const [camera, setCamera] = useState(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [activeMatch, setActiveMatch] = useState(null)
  const [matchLoading, setMatchLoading] = useState(false)
  const [startDialogOpen, setStartDialogOpen] = useState(false)
  const [startForm, setStartForm] = useState({
    matchType: 'standard',
    playerOneName: '',
    playerTwoName: '',
    playerOneRaceTo: 5,
    playerTwoRaceTo: 5,
    practiceTargetRacks: 0,
    playerOneRating: '',
    playerTwoRating: '',
    playerOneRatingType: 'Fargo',
    playerTwoRatingType: 'Fargo',
    matchDescription: '',
  })
  const [startError, setStartError] = useState('')
  const [scoreUpdating, setScoreUpdating] = useState(false)
  const [rtmpDialogOpen, setRtmpDialogOpen] = useState(false)
  const [rtmpUrl, setRtmpUrl] = useState('')
  const [rtmpError, setRtmpError] = useState('')
  const [rtmpStarting, setRtmpStarting] = useState(false)
  const [rtmpActive, setRtmpActive] = useState(false)
  const [rtmpStopping, setRtmpStopping] = useState(false)
  const [streamUrl, setStreamUrl] = useState('')
  const [streamError, setStreamError] = useState(false)
  const [previewLoaded, setPreviewLoaded] = useState(false)
  const [cameraMatches, setCameraMatches] = useState([])
  const [downloadingGame, setDownloadingGame] = useState(null)
  const [downloadingRecent, setDownloadingRecent] = useState(null)
  const [downloadError, setDownloadError] = useState('')
  const [startPracticeLoading, setStartPracticeLoading] = useState(false)

  useEffect(() => {
    if (!camera?.id) return
    if (rtmpActive) {
      setStreamUrl('')
      setPreviewLoaded(false)
      return
    }
    setStreamError(false)
    setStreamUrl(`/api/cameras/${camera.id}/stream`)
    setPreviewLoaded(false)
  }, [camera?.id, rtmpActive])

  const fetchActiveMatch = useCallback(async () => {
    if (!camera?.id) return
    setMatchLoading(true)
    try {
      const m = await getActiveMatch(camera.id)
      setActiveMatch(m)
    } catch {
      setActiveMatch(null)
    } finally {
      setMatchLoading(false)
    }
  }, [camera?.id])

  const fetchCamera = useCallback(async () => {
    if (!id) return
    setError('')
    try {
      const data = await getCamera(id)
      setCamera(data)
    } catch (err) {
      setError(err.message)
    }
  }, [id])

  useEffect(() => {
    if (!id) return
    let cancelled = false
    setLoading(true)
    async function fetch() {
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

  // Refresh camera periodically to update connection status
  useEffect(() => {
    if (!camera?.id) return
    const interval = setInterval(fetchCamera, 20000)
    return () => clearInterval(interval)
  }, [camera?.id, fetchCamera])

  useEffect(() => {
    if (camera?.id) fetchActiveMatch()
  }, [camera?.id, fetchActiveMatch])

  // Poll for active match updates when a match is in progress (keeps overlay score in sync)
  useEffect(() => {
    if (!camera?.id || !activeMatch?.id) return
    const interval = setInterval(fetchActiveMatch, 3000)
    return () => clearInterval(interval)
  }, [camera?.id, activeMatch?.id, fetchActiveMatch])

  const fetchCameraMatches = useCallback(async () => {
    if (!camera?.id) return
    try {
      const data = await listMatches()
      const forCamera = (data || []).filter((m) => m.camera_id === camera.id)
      setCameraMatches(forCamera.sort((a, b) => (b.start_time || 0) - (a.start_time || 0)))
    } catch {
      setCameraMatches([])
    }
  }, [camera?.id])

  useEffect(() => {
    if (camera?.id) fetchCameraMatches()
  }, [camera?.id, fetchCameraMatches])

  useEffect(() => {
    if (!camera) return
    const title = locationName ? `${locationName} – ${camera.name}` : camera.name
    document.title = `${title} | Table TV`
    return () => { document.title = 'Table TV' }
  }, [camera, locationName])

  const fetchRtmpStatus = useCallback(async () => {
    if (!camera?.id) return
    try {
      const { active } = await getRtmpStreamStatus(camera.id)
      setRtmpActive(active)
    } catch {
      setRtmpActive(false)
    }
  }, [camera?.id])

  useEffect(() => {
    if (!camera?.id) return
    fetchRtmpStatus()
    const interval = setInterval(fetchRtmpStatus, 5000)
    return () => clearInterval(interval)
  }, [camera?.id, fetchRtmpStatus])

  const handleStartPractice = async () => {
    if (!camera?.id) return
    setStartPracticeLoading(true)
    setStartError('')
    try {
      const { id: matchId } = await createMatch({
        match_type: 'practice',
        player_one: { name: 'Player', race_to: 0 },
        camera_id: camera.id,
      })
      await fetchActiveMatch()
      fetchCameraMatches()
      navigate(`/match/${matchId}`)
    } catch (err) {
      setStartError(err.message)
    } finally {
      setStartPracticeLoading(false)
    }
  }

  const handleStartMatch = async () => {
    const { matchType, playerOneName, playerTwoName, playerOneRaceTo, playerTwoRaceTo, practiceTargetRacks, playerOneRating, playerTwoRating, playerOneRatingType, playerTwoRatingType, matchDescription } = startForm
    const isPractice = matchType === 'practice'
    if (!playerOneName.trim()) {
      setStartError('Player name is required')
      return
    }
    if (!isPractice && !playerTwoName.trim()) {
      setStartError('Both player names are required')
      return
    }
    if (isPractice) {
      if (practiceTargetRacks < 0 || practiceTargetRacks > 21) {
        setStartError('Target racks must be 0 (no limit) or 1–21')
        return
      }
    } else if (playerOneRaceTo < 1 || playerOneRaceTo > 21 || playerTwoRaceTo < 1 || playerTwoRaceTo > 21) {
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
      const payload = {
        player_one: {
          name: playerOneName.trim(),
          race_to: isPractice ? practiceTargetRacks : playerOneRaceTo,
          ...(r1 != null && { rating: { type: playerOneRatingType, value: r1 } }),
        },
        camera_id: camera.id,
        ...(matchDescription.trim() && { description: matchDescription.trim() }),
      }
      if (isPractice) {
        payload.match_type = 'practice'
      } else {
        payload.player_two = {
          name: playerTwoName.trim(),
          race_to: playerTwoRaceTo,
          ...(r2 != null && { rating: { type: playerTwoRatingType, value: r2 } }),
        }
      }
      const { id: matchId } = await createMatch(payload)
      setStartDialogOpen(false)
      setStartForm({ matchType: 'standard', playerOneName: '', playerTwoName: '', playerOneRaceTo: 5, playerTwoRaceTo: 5, practiceTargetRacks: 0, playerOneRating: '', playerTwoRating: '', playerOneRatingType: 'Fargo', playerTwoRatingType: 'Fargo', matchDescription: '' })
      navigate(`/match/${matchId}`)
    } catch (err) {
      setStartError(err.message)
    }
  }

  const handleScoreChange = async (player, delta) => {
    if (!activeMatch || scoreUpdating) return
    const isPractice = activeMatch.match_type === 'practice'
    const p = player === 1 ? activeMatch.player_one : activeMatch.player_two
    const next = isPractice && p.race_to === 0
      ? Math.max(0, p.games_won + delta)
      : Math.max(0, Math.min(p.race_to || 21, p.games_won + delta))
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
      fetchCameraMatches()
    } catch {
      setActiveMatch(activeMatch)
    } finally {
      setScoreUpdating(false)
    }
  }

  const handleStartRtmp = async () => {
    const url = rtmpUrl.trim()
    if (!url.startsWith('rtmp://') && !url.startsWith('rtmps://')) {
      setRtmpError('Enter a valid RTMP URL (e.g. rtmp://a.rtmp.youtube.com/live2/xxxx)')
      return
    }
    setRtmpError('')
    setRtmpStarting(true)
    try {
      await startRtmpStream(camera.id, url)
      setRtmpActive(true)
      setRtmpDialogOpen(false)
      setRtmpUrl('')
    } catch (err) {
      setRtmpError(err.message)
    } finally {
      setRtmpStarting(false)
    }
  }

  const handleStopRtmp = async () => {
    if (!camera?.id || rtmpStopping) return
    setRtmpStopping(true)
    try {
      await stopRtmpStream(camera.id)
      setRtmpActive(false)
    } catch (err) {
      setRtmpError(err.message)
    } finally {
      setRtmpStopping(false)
    }
  }

  const handleEndMatch = async () => {
    if (!activeMatch || scoreUpdating) return
    setScoreUpdating(true)
    try {
      await endMatch(activeMatch.id)
      await fetchActiveMatch()
      fetchCameraMatches()
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

  const { label } = formatCameraType(camera.camera_type, 'labelNoUrl')
  const hasStream = !!camera

  return (
    <Box sx={{ p: 2 }}>
      <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
        Back to Home
      </Button>
      <Paper sx={{ p: 3 }}>
        <Box display="flex" alignItems="center" gap={2} sx={{ mb: 2 }}>
          <Typography variant="h4" component="h1">
            {locationName ? `${locationName} – ${camera.name}` : camera.name}
          </Typography>
          <Chip label={label} size="small" />
          {camera.connection_status === true && (
            <Chip label="Connected" color="success" size="small" />
          )}
          {camera.connection_status === false && (
            <Chip label="Offline" size="small" variant="outlined" color="default" />
          )}
        </Box>
        {hasStream && camera.connection_status === false && (
          <Alert severity="warning" sx={{ mt: 2 }}>
            Camera is offline. Streaming, practice, and matches are disabled until the camera reconnects.
          </Alert>
        )}

        {hasStream && camera.connection_status !== false && (
          <>
            <Box sx={{ mt: 2, position: 'relative', width: '100%' }}>
              <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap' }}>
                <Button
                  startIcon={<LiveTvIcon />}
                  variant="outlined"
                  onClick={() => { fetchRtmpStatus(); setRtmpDialogOpen(true) }}
                  disabled={rtmpActive}
                >
                  Go Live
                </Button>
                {rtmpActive && (
                  <Button
                    startIcon={<StopIcon />}
                    variant="outlined"
                    color="error"
                    onClick={handleStopRtmp}
                    disabled={rtmpStopping}
                  >
                    {rtmpStopping ? 'Stopping…' : 'Stop stream'}
                  </Button>
                )}
              </Box>
              <StreamPreview
                streamUrl={streamUrl}
                streamError={streamError}
                previewLoaded={previewLoaded}
                setPreviewLoaded={setPreviewLoaded}
                onRetry={() => {
                  setStreamError(false)
                  setPreviewLoaded(false)
                  setStreamUrl(`/api/cameras/${camera.id}/stream`)
                }}
                onStreamError={() => setStreamError(true)}
                rtmpActive={rtmpActive}
                cameraName={camera.name}
                locationName={locationName}
                overlayMatch={null}
              />
              <Box sx={{ mt: 2, display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
                <Typography variant="body2" color="text.secondary" sx={{ flexBasis: { xs: '100%', sm: 'auto' } }}>
                  Download last:
                </Typography>
                {[30, 60, 90].map((sec) => (
                  <DownloadRecordingButton
                    key={sec}
                    cameraId={camera.id}
                    startMs={() => Date.now() - sec * 1000}
                    durationSec={sec}
                    filename={`clip-${sec}s.mp4`}
                    disabled={downloadingRecent !== null}
                    onLoadingStart={() => setDownloadingRecent(sec)}
                    onLoadingEnd={() => setDownloadingRecent(null)}
                    onError={(err) => setDownloadError(err.message || 'Download failed')}
                    label={`${sec}s`}
                    variant="outlined"
                  />
                ))}
              </Box>
            </Box>

            <Box sx={{ mt: 3, pt: 2, borderTop: 1, borderColor: 'divider' }}>
              {downloadError && (
                <Alert severity="error" sx={{ mb: 2 }} onClose={() => setDownloadError('')}>
                  {downloadError}
                </Alert>
              )}
              {startError && !startDialogOpen && (
                <Alert severity="error" sx={{ mb: 2 }} onClose={() => setStartError('')}>
                  {startError}
                </Alert>
              )}
              <Box sx={{ mb: 2, display: 'flex', alignItems: 'center', gap: 2, flexWrap: 'wrap' }}>
                <Box>
                  <Typography variant="h6">
                    Pool Match
                  </Typography>
                  {activeMatch?.started_by && (
                    <Typography variant="body2" color="text.secondary">
                      Started by {activeMatch.started_by}
                    </Typography>
                  )}
                </Box>
                {activeMatch?.can_edit && (
                  <Button
                    startIcon={<EditIcon />}
                    variant="outlined"
                    size="small"
                    onClick={() => navigate(`/match/${activeMatch.id}`)}
                  >
                    Edit details
                  </Button>
                )}
              </Box>
              {matchLoading ? (
                <CircularProgress size={24} />
              ) : activeMatch ? (
                <Stack spacing={2}>
                  {activeMatch.description?.trim() && (
                    <Box sx={{ py: 1, px: 2, bgcolor: 'action.hover', borderRadius: 1 }}>
                      <Typography variant="body2" sx={{ whiteSpace: 'pre-wrap' }}>
                        {activeMatch.description.trim()}
                      </Typography>
                    </Box>
                  )}
                  <MatchScoreControls
                    match={activeMatch}
                    scoreUpdating={scoreUpdating}
                    onScoreChange={handleScoreChange}
                    onEndMatch={handleEndMatch}
                  />
                </Stack>
              ) : (
                <Stack direction="row" spacing={2}>
                  <Button
                    startIcon={<PlayArrowIcon />}
                    variant="outlined"
                    onClick={handleStartPractice}
                    disabled={startPracticeLoading}
                  >
                    {startPracticeLoading ? 'Starting…' : 'Start practice'}
                  </Button>
                  <Button
                    startIcon={<PlayArrowIcon />}
                    variant="contained"
                    onClick={() => setStartDialogOpen(true)}
                  >
                    Start match
                  </Button>
                </Stack>
              )}
            </Box>

          </>
        )}
        <Box sx={{ mt: 3, pt: 2, borderTop: 1, borderColor: 'divider' }}>
          <RecordingTimelineBar
            cameraId={camera.id}
            recordDeleteAfter={recordDeleteAfter}
          />
        </Box>
        {cameraMatches.length > 0 && (
          <MatchHistory matches={cameraMatches} recordDeleteAfter={recordDeleteAfter} onError={(err) => setDownloadError(err.message || 'Download failed')} />
        )}
      </Paper>

      <Dialog open={rtmpDialogOpen} onClose={() => setRtmpDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Go Live</DialogTitle>
        <DialogContent>
          {rtmpActive && (
            <Alert severity="info" sx={{ mb: 2 }}>
              Stream is live. Click &quot;Stop stream&quot; below to end the broadcast.
            </Alert>
          )}
          {rtmpError && (
            <Alert severity="error" sx={{ mb: 2 }} onClose={() => setRtmpError('')}>
              {rtmpError}
            </Alert>
          )}
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            Push the stream to YouTube Live or other RTMP destinations.
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
            disabled={rtmpStarting}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRtmpDialogOpen(false)}>Cancel</Button>
          {rtmpActive && (
            <Button
              variant="outlined"
              color="error"
              onClick={handleStopRtmp}
              disabled={rtmpStopping}
            >
              {rtmpStopping ? 'Stopping…' : 'Stop stream'}
            </Button>
          )}
          <Button variant="contained" onClick={handleStartRtmp} disabled={rtmpStarting || rtmpActive}>
            {rtmpStarting ? 'Starting…' : 'Start stream'}
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={startDialogOpen} onClose={() => setStartDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Start pool match</DialogTitle>
        <DialogContent>
          {startError && (
            <Alert severity="error" sx={{ mb: 2 }} onClose={() => setStartError('')}>
              {startError}
            </Alert>
          )}
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
          <TextField
            label="Match description (optional)"
            placeholder="Included in live video post. Supports multiple lines."
            value={startForm.matchDescription}
            onChange={(e) => setStartForm((f) => ({ ...f, matchDescription: e.target.value }))}
            fullWidth
            multiline
            minRows={3}
            sx={{ mt: 2 }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setStartDialogOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            onClick={handleStartMatch}
            disabled={camera.connection_status === false}
          >
            Start
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  )
}
