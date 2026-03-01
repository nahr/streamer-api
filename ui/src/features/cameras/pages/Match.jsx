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
import HistoryIcon from '@mui/icons-material/History'
import StopIcon from '@mui/icons-material/Stop'
import LiveTvIcon from '@mui/icons-material/LiveTv'
import VideocamIcon from '@mui/icons-material/Videocam'
import { getCamera, getFacebookLiveUrl, getFacebookStatus, getRtmpStreamStatus, parseCameraType, startRtmpStream, stopRtmpStream } from '../api/cameras.js'
import { getMatch, updateScore, endMatch } from '../api/poolMatches.js'
import { useApiInfo } from '../../../apiInfoStore.jsx'
import { getToken, urlWithToken } from '../../../apiClient.js'

function LiveTimestamp() {
  const [now, setNow] = useState(() => new Date())
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000)
    return () => clearInterval(id)
  }, [])
  return now.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function formatTime(ms) {
  const d = new Date(ms)
  return d.toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  })
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
  const [goLivePrivacy, setGoLivePrivacy] = useState('EVERYONE')
  const [streamUrl, setStreamUrl] = useState('')
  const [streamError, setStreamError] = useState(false)
  const [previewLoaded, setPreviewLoaded] = useState(false)

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
    const camType = parseCameraType(camera?.camera_type).type
    if (!camera?.id || (camType !== 'internal' && camType !== 'rtsp')) return
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
  }, [camera?.id, camera?.camera_type])

  const fetchRtmpStatus = useCallback(async () => {
    if (!camera?.id) return
    const camType = parseCameraType(camera?.camera_type).type
    if (camType !== 'internal' && camType !== 'rtsp') return
    try {
      const { active } = await getRtmpStreamStatus(camera.id)
      setRtmpActive(active)
    } catch {
      setRtmpActive(false)
    }
  }, [camera?.id, camera?.camera_type])

  useEffect(() => {
    if (!camera?.id) return
    const camType = parseCameraType(camera?.camera_type).type
    if (camType !== 'internal' && camType !== 'rtsp') return
    fetchRtmpStatus()
    const interval = setInterval(fetchRtmpStatus, 5000)
    return () => clearInterval(interval)
  }, [camera?.id, camera?.camera_type, fetchRtmpStatus])

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
    const p = player === 1 ? match.player_one : match.player_two
    const next = Math.max(0, Math.min(p.race_to, p.games_won + delta))
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
        const title = `${prefix}: ${match.player_one.name} vs ${match.player_two.name}`
        const formatRating = (p) => p.rating ? `${p.rating.type} ${p.rating.value}` : null
        const p1Part = formatRating(match.player_one)
          ? `${match.player_one.name} (${formatRating(match.player_one)})`
          : match.player_one.name
        const p2Part = formatRating(match.player_two)
          ? `${match.player_two.name} (${formatRating(match.player_two)})`
          : match.player_two.name
        const headerLine = `${p1Part} vs ${p2Part}`
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
      }
    },
    [camera?.id, camera?.name, match, locationName]
  )

  useEffect(() => {
    const authKey = searchParams.get('auth_key')
    if (!authKey || !id || !camera?.id || !match) return
    setSearchParams({}, { replace: true })
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
    const title = locationName
      ? `${locationName} – ${match.player_one.name} vs ${match.player_two.name}`
      : `${match.player_one.name} vs ${match.player_two.name}`
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

  const score = `${match.player_one.games_won} - ${match.player_two.games_won}`
  const isActive = !match.end_time
  const winner = isActive
    ? null
    : match.player_one.games_won >= match.player_one.race_to
      ? match.player_one.name
      : match.player_two.games_won >= match.player_two.race_to
        ? match.player_two.name
        : null
  const parsed = parseCameraType(camera?.camera_type)
  const hasStream = camera && (parsed.type === 'internal' || parsed.type === 'rtsp')

  return (
    <Box sx={{ p: 2 }}>
      <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
        Back to Home
      </Button>
      <Paper sx={{ p: 3 }}>
        <Box display="flex" alignItems="center" gap={2} sx={{ mb: 2 }}>
          <Typography variant="h4" component="h1">
            {match.player_one.name} vs {match.player_two.name}
          </Typography>
          <Typography variant="h5" component="span" color="primary" fontWeight={600}>
            {score}
          </Typography>
          {isActive && <Chip label="In progress" color="primary" size="small" />}
          {match.end_time && (
            <Chip
              label={winner ? `${winner} won` : 'Ended early'}
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

        {hasStream && (
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
            <Box sx={{ position: 'relative', display: 'inline-block' }}>
              {rtmpActive && parsed.type === 'internal' ? (
                <Box
                  sx={{
                    width: '100%',
                    maxWidth: 640,
                    height: 360,
                    borderRadius: 8,
                    backgroundColor: '#000',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'rgba(255,255,255,0.7)',
                  }}
                >
                  <Typography>Stream is live — preview unavailable</Typography>
                </Box>
              ) : streamError ? (
                <Box
                  sx={{
                    width: '100%',
                    maxWidth: 640,
                    aspectRatio: '16/9',
                    borderRadius: 8,
                    backgroundColor: '#000',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'rgba(255,255,255,0.7)',
                    flexDirection: 'column',
                    gap: 1,
                  }}
                >
                  <Typography>Stream unavailable</Typography>
                  <Typography variant="body2" sx={{ opacity: 0.8 }}>
                    {parsed.type === 'rtsp'
                      ? 'Check that the RTSP URL is valid and reachable'
                      : 'Ensure you are logged in and the camera is available'}
                  </Typography>
                  <Button
                    size="small"
                    variant="outlined"
                    onClick={() => { setStreamError(false); setPreviewLoaded(false); getToken().then((t) => setStreamUrl(urlWithToken(`/api/cameras/${camera.id}/stream`, t))) }}
                    sx={{ mt: 1 }}
                  >
                    Retry
                  </Button>
                </Box>
              ) : !streamUrl ? (
                <Box
                  sx={{
                    width: '100%',
                    maxWidth: 640,
                    aspectRatio: '16/9',
                    borderRadius: 8,
                    backgroundColor: '#000',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'rgba(255,255,255,0.7)',
                    flexDirection: 'column',
                    gap: 1,
                  }}
                >
                  <CircularProgress size={24} />
                  <Typography variant="body2">Connecting to stream…</Typography>
                </Box>
              ) : (
                <>
                  <img
                    src={streamUrl}
                    alt={`${camera.name} live stream`}
                    onLoad={() => setPreviewLoaded(true)}
                    onError={() => setStreamError(true)}
                    style={{
                      width: '100%',
                      maxWidth: 640,
                      borderRadius: 8,
                      backgroundColor: '#000',
                      display: 'block',
                    }}
                  />
                  {previewLoaded && (
                    <>
                      <Box
                        sx={{
                          position: 'absolute',
                          top: 8,
                          left: 8,
                          background: 'rgba(0,0,0,0.7)',
                          color: '#fff',
                          px: 1.5,
                          py: 1,
                          borderRadius: 1,
                          fontSize: '0.875rem',
                        }}
                      >
                        {locationName && (
                          <Box component="div" sx={{ fontWeight: 600, mb: 0.25 }}>
                            {locationName}
                          </Box>
                        )}
                        <Box component="div" sx={{ fontSize: '0.75rem', opacity: 0.9 }}>
                          {camera.name}
                        </Box>
                      </Box>
                      <Box
                        sx={{
                          position: 'absolute',
                          top: 8,
                          right: 8,
                          background: 'rgba(0,0,0,0.7)',
                          color: '#fff',
                          px: 1.5,
                          py: 1,
                          borderRadius: 1,
                          fontSize: '0.875rem',
                          fontVariantNumeric: 'tabular-nums',
                        }}
                      >
                        <LiveTimestamp />
                      </Box>
                      {isActive && (
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
                              {match.player_one.name}
                            </Typography>
                            {match.player_one.rating && (
                              <Typography variant="caption" color="rgba(255,255,255,0.8)">
                                {match.player_one.rating.type} {match.player_one.rating.value}
                              </Typography>
                            )}
                          </Box>
                          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, flexShrink: 0 }}>
                            <Box sx={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center', minWidth: 32, height: 32, borderRadius: '50%', border: '2px solid #fff' }}>
                              <Typography variant="subtitle1" fontWeight={700}>{match.player_one.games_won}</Typography>
                            </Box>
                            <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center' }}>
                              <Typography variant="caption" color="rgba(255,255,255,0.8)">race to</Typography>
                              <Typography variant="caption" color="rgba(255,255,255,0.8)">
                                {match.player_one.race_to}/{match.player_two.race_to}
                              </Typography>
                            </Box>
                            <Box sx={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center', minWidth: 32, height: 32, borderRadius: '50%', border: '2px solid #fff' }}>
                              <Typography variant="subtitle1" fontWeight={700}>{match.player_two.games_won}</Typography>
                            </Box>
                          </Box>
                          <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', minWidth: 0, flex: 1 }}>
                            <Typography variant="subtitle2" fontWeight={600} noWrap sx={{ maxWidth: '100%' }}>
                              {match.player_two.name}
                            </Typography>
                            {match.player_two.rating && (
                              <Typography variant="caption" color="rgba(255,255,255,0.8)">
                                {match.player_two.rating.type} {match.player_two.rating.value}
                              </Typography>
                            )}
                          </Box>
                        </Box>
                      )}
                    </>
                  )}
                </>
              )}
            </Box>
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
            <Stack spacing={2}>
              <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} alignItems="center" flexWrap="wrap">
                <Box display="flex" alignItems="center" gap={0.5}>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(1, -1)}
                    disabled={scoreUpdating || match.player_one.games_won === 0}
                    aria-label="Decrease player 1 score"
                  >
                    <RemoveIcon />
                  </IconButton>
                  <Typography variant="h5" component="span" sx={{ minWidth: 40, textAlign: 'center' }}>
                    {match.player_one.games_won}
                  </Typography>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(1, 1)}
                    disabled={scoreUpdating || match.player_one.games_won >= match.player_one.race_to}
                    aria-label="Increase player 1 score"
                  >
                    <AddIcon />
                  </IconButton>
                  <Typography sx={{ ml: 1 }}>{match.player_one.name}</Typography>
                  <Typography color="text.secondary" variant="body2" sx={{ ml: 0.5 }}>
                    (race to {match.player_one.race_to})
                  </Typography>
                </Box>
                <Typography color="text.secondary">vs</Typography>
                <Box display="flex" alignItems="center" gap={0.5}>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(2, -1)}
                    disabled={scoreUpdating || match.player_two.games_won === 0}
                    aria-label="Decrease player 2 score"
                  >
                    <RemoveIcon />
                  </IconButton>
                  <Typography variant="h5" component="span" sx={{ minWidth: 40, textAlign: 'center' }}>
                    {match.player_two.games_won}
                  </Typography>
                  <IconButton
                    size="small"
                    onClick={() => handleScoreChange(2, 1)}
                    disabled={scoreUpdating || match.player_two.games_won >= match.player_two.race_to}
                    aria-label="Increase player 2 score"
                  >
                    <AddIcon />
                  </IconButton>
                  <Typography sx={{ ml: 1 }}>{match.player_two.name}</Typography>
                  <Typography color="text.secondary" variant="body2" sx={{ ml: 0.5 }}>
                    (race to {match.player_two.race_to})
                  </Typography>
                </Box>
              </Stack>
              <Button
                startIcon={<StopIcon />}
                variant="outlined"
                color="secondary"
                onClick={handleEndMatch}
                disabled={scoreUpdating}
              >
                End match early
              </Button>
            </Stack>
          ) : (
            <Typography color="text.secondary" variant="body2">
              {match.player_one.games_won >= match.player_one.race_to
                ? `${match.player_one.name} won`
                : match.player_two.games_won >= match.player_two.race_to
                  ? `${match.player_two.name} won`
                  : 'Match ended early'}
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
              {match.score_history.map((entry, i) => (
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
                  <Typography variant="body1" fontWeight={600}>
                    {entry.player_one_games_won} – {entry.player_two_games_won}
                  </Typography>
                </Box>
              ))}
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
                  onChange={(e) => setGoLivePrivacy(e.target.value)}
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
    </Box>
  )
}
