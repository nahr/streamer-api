import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate, useSearchParams } from 'react-router-dom'
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
  Stack,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
} from '@mui/material'
import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import HistoryIcon from '@mui/icons-material/History'
import DownloadIcon from '@mui/icons-material/Download'
import StopIcon from '@mui/icons-material/Stop'
import LiveTvIcon from '@mui/icons-material/LiveTv'
import VideocamIcon from '@mui/icons-material/Videocam'
import { getCamera, getFacebookLiveUrl, getFacebookStatus, getRtmpStreamStatus, startRtmpStream, stopRtmpStream } from '../api/cameras.js'
import { getMatch, updateScore, endMatch, downloadGameRecording } from '../api/poolMatches.js'
import { useApiInfo } from '../../../apiInfoStore.jsx'
import { getToken, urlWithToken } from '../../../apiClient.js'
import { MatchDuration } from '../../../components/MatchDuration.jsx'
import { StreamPreview } from '../components/StreamPreview.jsx'
import { MatchScoreControls } from '../components/MatchScoreControls.jsx'
import { formatTime, formatMatchWinner, formatMatchTitle, getMatchWinner } from '../../../utils/format.js'

export function Match() {
  const { id } = useParams()
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const { locationName } = useApiInfo()
  const [match, setMatch] = useState(null)
  const [camera, setCamera] = useState(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [scoreUpdating, setScoreUpdating] = useState(false)
  const [rtmpDialogOpen, setRtmpDialogOpen] = useState(false)
  const [rtmpUrl, setRtmpUrl] = useState('')
  const [rtmpError, setRtmpError] = useState('')
  const [rtmpStarting, setRtmpStarting] = useState(false)
  const [rtmpActive, setRtmpActive] = useState(false)
  const [rtmpStopping, setRtmpStopping] = useState(false)
  const [facebookConfigured, setFacebookConfigured] = useState(false)
  const [goLivePrivacy, setGoLivePrivacy] = useState(() => {
    const saved = localStorage.getItem('table-tv-go-live-privacy')
    return ['EVERYONE', 'ALL_FRIENDS', 'FRIENDS_OF_FRIENDS', 'SELF'].includes(saved) ? saved : 'EVERYONE'
  })
  const [isFacebookLiveFlow, setIsFacebookLiveFlow] = useState(false)
  const [streamUrl, setStreamUrl] = useState('')
  const [streamError, setStreamError] = useState(false)
  const [previewLoaded, setPreviewLoaded] = useState(false)
  const [downloadingGame, setDownloadingGame] = useState(null)

  useEffect(() => {
    if (!id) return
    let cancelled = false
    async function fetch() {
      setLoading(true)
      setError('')
      try {
        const data = await getMatch(id)
        if (!cancelled) setMatch(data)
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
    if (!match?.camera_id) {
      setCamera(null)
      return
    }
    let cancelled = false
    async function fetch() {
      try {
        const data = await getCamera(match.camera_id)
        if (!cancelled) setCamera(data)
      } catch {
        if (!cancelled) setCamera(null)
      }
    }
    fetch()
    return () => { cancelled = true }
  }, [match?.camera_id])

  useEffect(() => {
    if (!camera?.id) return
    if (rtmpActive) {
      setStreamUrl('')
      setPreviewLoaded(false)
      return
    }
    setStreamError(false)
    setStreamUrl('')
    setPreviewLoaded(false)
    let cancelled = false
    getToken().then((token) => {
      if (!cancelled) {
        setStreamUrl(urlWithToken(`/api/cameras/${camera.id}/stream`, token))
      }
    })
    return () => { cancelled = true }
  }, [camera?.id, rtmpActive])

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

  useEffect(() => {
    let cancelled = false
    async function check() {
      try {
        const { configured } = await getFacebookStatus()
        if (!cancelled) setFacebookConfigured(configured)
      } catch {
        if (!cancelled) setFacebookConfigured(false)
      }
    }
    check()
    return () => { cancelled = true }
  }, [])

  const handleScoreChange = async (player, delta) => {
    if (!match || scoreUpdating || match.end_time) return
    const isPractice = match.match_type === 'practice'
    const p = player === 1 ? match.player_one : match.player_two
    const next = isPractice && p.race_to === 0
      ? Math.max(0, p.games_won + delta)
      : Math.max(0, Math.min(p.race_to || 21, p.games_won + delta))
    if (next === p.games_won) return
    setScoreUpdating(true)
    setMatch((prev) => {
      if (!prev) return prev
      return {
        ...prev,
        player_one: player === 1 ? { ...prev.player_one, games_won: next } : prev.player_one,
        player_two: player === 2 ? { ...prev.player_two, games_won: next } : prev.player_two,
      }
    })
    try {
      const updated = await updateScore(match.id, player, next)
      setMatch(updated)
    } catch {
      setMatch(match)
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

  const handleGoLiveFacebook = () => {
    sessionStorage.setItem('table-tv-go-live-privacy', goLivePrivacy)
    const returnTo = `/match/${id}`
    window.location.href = `/api/facebook/auth?return_to=${encodeURIComponent(returnTo)}`
  }

  const runFacebookLiveWithAuthKey = useCallback(
    async (authKey) => {
      if (!camera?.id || !match) return
      setRtmpError('')
      setRtmpStarting(true)
      try {
        const prefix = locationName ? `${locationName} - ${camera.name}` : camera.name
        const title = match.match_type === 'practice'
          ? `${prefix}: Practice: ${match.player_one.name}`
          : `${prefix}: ${match.player_one.name} vs ${match.player_two.name}`
        const formatRating = (p) => p.rating ? `${p.rating.type} ${p.rating.value}` : null
        const headerLine = match.match_type === 'practice'
          ? `Practice: ${formatRating(match.player_one) ? `${match.player_one.name} (${formatRating(match.player_one)})` : match.player_one.name}`
          : `${formatRating(match.player_one) ? `${match.player_one.name} (${formatRating(match.player_one)})` : match.player_one.name} vs ${formatRating(match.player_two) ? `${match.player_two.name} (${formatRating(match.player_two)})` : match.player_two.name}`
        const desc = match.description?.trim()
        const description = desc ? `${headerLine}\n${desc}` : headerLine
        const privacy = sessionStorage.getItem('table-tv-go-live-privacy') || 'EVERYONE'
        sessionStorage.removeItem('table-tv-go-live-privacy')
        const { url } = await getFacebookLiveUrl({ title, description, privacy, auth_key: authKey })
        await startRtmpStream(camera.id, url)
        setRtmpActive(true)
        setRtmpDialogOpen(false)
        setRtmpUrl('')
      } catch (err) {
        setRtmpError(err.message)
      } finally {
        setRtmpStarting(false)
        setIsFacebookLiveFlow(false)
      }
    },
    [camera?.id, camera?.name, match, locationName]
  )

  useEffect(() => {
    const authKey = searchParams.get('auth_key')
    if (!authKey || !id || !camera?.id || !match) return
    setSearchParams({}, { replace: true })
    setIsFacebookLiveFlow(true)
    setRtmpDialogOpen(true)
    runFacebookLiveWithAuthKey(authKey)
  }, [searchParams, id, camera?.id, match, setSearchParams, runFacebookLiveWithAuthKey])

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
    if (!match || scoreUpdating || match.end_time) return
    setScoreUpdating(true)
    try {
      const updated = await endMatch(match.id)
      setMatch(updated)
    } finally {
      setScoreUpdating(false)
    }
  }

  useEffect(() => {
    if (!match) return
    const matchTitle = formatMatchTitle(match)
    const title = locationName ? `${locationName} – ${matchTitle}` : matchTitle
    document.title = `${title} | Table TV`
    return () => { document.title = 'Table TV' }
  }, [match, locationName])

  if (loading) {
    return (
      <Box display="flex" justifyContent="center" py={4}>
        <CircularProgress />
      </Box>
    )
  }

  if (error || !match) {
    return (
      <Box sx={{ p: 2 }}>
        <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
          Back
        </Button>
        <Typography color="error">{error || 'Match not found'}</Typography>
      </Box>
    )
  }

  const score = match.match_type === 'practice'
    ? `${match.player_one.games_won} rack${match.player_one.games_won !== 1 ? 's' : ''}`
    : `${match.player_one.games_won} - ${match.player_two.games_won}`
  const isActive = !match.end_time
  const winner = getMatchWinner(match)
  const hasStream = !!camera

  return (
    <Box sx={{ p: 2 }}>
      <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
        Back to Home
      </Button>
      <Paper sx={{ p: 3 }}>
        <Box display="flex" alignItems="center" gap={2} sx={{ mb: 2 }}>
          <Typography variant="h4" component="h1">
            {formatMatchTitle(match)}
          </Typography>
          <Typography variant="h5" component="span" color="primary" fontWeight={600}>
            {score}
          </Typography>
          {isActive && <Chip label="In progress" color="primary" size="small" />}
          {match.end_time && (
            <Chip
              label={formatMatchWinner(match)}
              color="default"
              size="small"
            />
          )}
        </Box>

        <Box sx={{ mb: 2 }}>
          <Typography variant="body2" color="text.secondary">
            {formatTime(match.start_time)}
            {match.end_time && ` – ${formatTime(match.end_time)}`}
            {' · '}
            <MatchDuration match={match} />
          </Typography>
          {match.started_by && (
            <Typography variant="body2" color="text.secondary">
              Started by {match.started_by}
            </Typography>
          )}
        </Box>

        {match.description?.trim() && (
          <Box sx={{ py: 1, px: 2, bgcolor: 'action.hover', borderRadius: 1, mb: 2 }}>
            <Typography variant="body2" sx={{ whiteSpace: 'pre-wrap' }}>
              {match.description.trim()}
            </Typography>
          </Box>
        )}

        {hasStream && camera && (
          <Box sx={{ mt: 2, position: 'relative', display: 'inline-block' }}>
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
                getToken().then((t) => setStreamUrl(urlWithToken(`/api/cameras/${camera.id}/stream`, t)))
              }}
              onStreamError={() => setStreamError(true)}
              rtmpActive={rtmpActive}
              cameraName={camera.name}
              locationName={locationName}
              overlayMatch={isActive ? match : null}
            />
          </Box>
        )}

        <Box sx={{ mt: 3, pt: 2, borderTop: 1, borderColor: 'divider' }}>
          <Box sx={{ mb: 2 }}>
            <Typography variant="h6">Match controls</Typography>
            {match.started_by && (
              <Typography variant="body2" color="text.secondary">
                Started by {match.started_by}
              </Typography>
            )}
          </Box>
          {isActive ? (
            <MatchScoreControls
              match={match}
              scoreUpdating={scoreUpdating}
              onScoreChange={handleScoreChange}
              onEndMatch={handleEndMatch}
            />
          ) : (
            <Typography color="text.secondary" variant="body2">
              {formatMatchWinner(match)}
            </Typography>
          )}
        </Box>

        {(match.score_history?.length ?? 0) > 0 && (
          <Box sx={{ mt: 2, pt: 2, borderTop: 1, borderColor: 'divider' }}>
            <Typography variant="h6" sx={{ mb: 1.5, display: 'flex', alignItems: 'center', gap: 1 }}>
              <HistoryIcon fontSize="small" />
              Score history
            </Typography>
            <Stack component="ul" spacing={0} sx={{ listStyle: 'none', pl: 0, m: 0 }}>
              {match.score_history.map((entry, i) => {
                const prev = i > 0 ? match.score_history[i - 1] : { player_one_games_won: 0, player_two_games_won: 0 }
                const p1Increased = entry.player_one_games_won > prev.player_one_games_won
                const player = p1Increased ? match.player_one.name : match.player_two.name
                const gameNumber = entry.player_one_games_won + entry.player_two_games_won
                const rackNumber = entry.player_one_games_won
                const startMs = i === 0 ? match.start_time : prev.timestamp
                const durationSec = Math.max(1, (entry.timestamp - startMs) / 1000)
                const isDownloading = downloadingGame === i
                const handleDownload = async () => {
                  if (!match.camera_id) return
                  setDownloadingGame(i)
                  try {
                    await downloadGameRecording(
                      match.camera_id,
                      startMs,
                      durationSec,
                      match.match_type === 'practice' ? `rack-${rackNumber}.mp4` : `game-${gameNumber}.mp4`
                    )
                  } catch (err) {
                    console.error('Download failed', err)
                  } finally {
                    setDownloadingGame(null)
                  }
                }
                return (
                  <Box
                    key={i}
                    component="li"
                    sx={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 2,
                      py: 1,
                      borderBottom: i < match.score_history.length - 1 ? 1 : 0,
                      borderColor: 'divider',
                    }}
                  >
                    <Typography variant="body2" color="text.secondary" sx={{ minWidth: 140 }}>
                      {formatTime(entry.timestamp)}
                    </Typography>
                    <Typography variant="body1" fontWeight={600} sx={{ flex: 1 }}>
                      {match.match_type === 'practice'
                        ? `Rack ${rackNumber}`
                        : `${player} won game ${gameNumber}, ${entry.player_one_games_won} – ${entry.player_two_games_won}`}
                    </Typography>
                    {match.camera_id && (
                      <Button
                        size="small"
                        startIcon={<DownloadIcon />}
                        onClick={handleDownload}
                        disabled={isDownloading}
                      >
                        {isDownloading ? 'Downloading…' : 'Download'}
                      </Button>
                    )}
                  </Box>
                )
              })}
            </Stack>
          </Box>
        )}

        {match.camera_id && (
          <Box sx={{ mt: 2, pt: 2, borderTop: 1, borderColor: 'divider' }}>
            <Typography variant="subtitle2" color="text.secondary" gutterBottom>
              Camera
            </Typography>
            <Button
              startIcon={<VideocamIcon />}
              variant="outlined"
              size="small"
              onClick={() => navigate(`/camera/${match.camera_id}`)}
            >
              {match.camera_name || 'View camera'}
            </Button>
          </Box>
        )}
      </Paper>

      <Dialog open={rtmpDialogOpen} onClose={() => setRtmpDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Go Live</DialogTitle>
        <DialogContent>
          {rtmpStarting && isFacebookLiveFlow ? (
            <Box display="flex" justifyContent="center" alignItems="center" py={4}>
              <CircularProgress />
            </Box>
          ) : (
            <>
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
            Push the stream to YouTube Live, Facebook, or other RTMP destinations.
            The match overlay (player names, ratings, score) is burned into the stream.
          </Typography>
          {facebookConfigured && (
            <>
              <FormControl fullWidth sx={{ mb: 2 }} disabled={rtmpStarting}>
                <InputLabel>Privacy</InputLabel>
                <Select
                  value={goLivePrivacy}
                  label="Privacy"
                  onChange={(e) => {
                  const v = e.target.value
                  setGoLivePrivacy(v)
                  localStorage.setItem('table-tv-go-live-privacy', v)
                }}
                >
                  <MenuItem value="EVERYONE">Public</MenuItem>
                  <MenuItem value="ALL_FRIENDS">Friends</MenuItem>
                  <MenuItem value="FRIENDS_OF_FRIENDS">Friends of friends</MenuItem>
                  <MenuItem value="SELF">Only me</MenuItem>
                </Select>
              </FormControl>
              <Button
                variant="outlined"
                fullWidth
                sx={{ mb: 1 }}
                onClick={handleGoLiveFacebook}
                disabled={rtmpStarting}
              >
                {rtmpStarting ? 'Starting…' : 'Go Live with Facebook'}
              </Button>
              <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 2 }}>
                You&apos;ll sign in with Facebook; the stream will appear on your profile.
              </Typography>
            </>
          )}
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            Or enter RTMP URL manually:
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
            </>
          )}
        </DialogContent>
        {!(rtmpStarting && isFacebookLiveFlow) && (
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
        )}
      </Dialog>
    </Box>
  )
}
