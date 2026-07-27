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
  Stack,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
} from '@mui/material'
import ArrowBackIcon from '@mui/icons-material/ArrowBack'
import EditIcon from '@mui/icons-material/Edit'
import HistoryIcon from '@mui/icons-material/History'
import StopIcon from '@mui/icons-material/Stop'
import LiveTvIcon from '@mui/icons-material/LiveTv'
import VideocamIcon from '@mui/icons-material/Videocam'
import { getCamera, getRtmpStreamStatus, startRtmpStream, stopRtmpStream } from '../api/cameras.js'
import { getMatch, updateScore, endMatch, updateMatchDetails } from '../api/poolMatches.js'
import { useApiInfo } from '../../../apiInfoStore.jsx'
import { MatchDuration } from '../../../components/MatchDuration.jsx'
import { StreamPreview } from '../components/StreamPreview.jsx'
import { MatchScoreControls } from '../components/MatchScoreControls.jsx'
import { DownloadRecordingButton } from '../components/DownloadRecordingButton.jsx'
import { MatchHistory } from '../components/MatchHistory.jsx'
import { formatTime, formatDuration, formatMatchWinner, formatMatchTitle, isRecordingAvailable, formatRecordingFilename } from '../../../utils/format.js'

export function Match() {
  const { id } = useParams()
  const navigate = useNavigate()
  const { locationName, recordDeleteAfter } = useApiInfo()
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
  const [streamUrl, setStreamUrl] = useState('')
  const [streamError, setStreamError] = useState(false)
  const [previewLoaded, setPreviewLoaded] = useState(false)
  const [downloadingGame, setDownloadingGame] = useState(null)
  const [downloadError, setDownloadError] = useState('')
  const [editDialogOpen, setEditDialogOpen] = useState(false)
  const [editSaving, setEditSaving] = useState(false)
  const [editError, setEditError] = useState('')
  const [editForm, setEditForm] = useState({})

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

  // Poll for match updates when match is active (keeps overlay score in sync)
  useEffect(() => {
    if (!id || !match?.id || match.end_time) return
    const interval = setInterval(async () => {
      try {
        const data = await getMatch(id)
        setMatch(data)
      } catch {
        // ignore
      }
    }, 3000)
    return () => clearInterval(interval)
  }, [id, match?.id, match?.end_time])

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
    setStreamUrl(`/api/cameras/${camera.id}/stream`)
    setPreviewLoaded(false)
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

  const openEditDialog = () => {
    setEditForm({
      name1: match.player_one.name,
      rating1Type: match.player_one.rating?.type || 'Apa',
      rating1Value: match.player_one.rating?.value ?? '',
      raceTo1: match.player_one.race_to,
      name2: match.player_two.name,
      rating2Type: match.player_two.rating?.type || 'Apa',
      rating2Value: match.player_two.rating?.value ?? '',
      raceTo2: match.player_two.race_to,
      description: match.description || '',
    })
    setEditError('')
    setEditDialogOpen(true)
  }

  const handleSaveEdit = async () => {
    if (!match || editSaving) return
    setEditSaving(true)
    setEditError('')
    try {
      const payload = {
        player_one: {
          name: editForm.name1?.trim() || undefined,
          race_to: editForm.raceTo1,
          rating: editForm.rating1Value !== '' && editForm.rating1Value !== undefined
            ? { type: editForm.rating1Type, value: Number(editForm.rating1Value) }
            : undefined,
        },
        player_two: match.match_type === 'standard' ? {
          name: editForm.name2?.trim() || undefined,
          race_to: editForm.raceTo2,
          rating: editForm.rating2Value !== '' && editForm.rating2Value !== undefined
            ? { type: editForm.rating2Type, value: Number(editForm.rating2Value) }
            : undefined,
        } : undefined,
        description: editForm.description?.trim() || undefined,
      }
      const updated = await updateMatchDetails(match.id, payload)
      setMatch(updated)
      setEditDialogOpen(false)
    } catch (err) {
      setEditError(err.message || 'Failed to update')
    } finally {
      setEditSaving(false)
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

  const rackCount = match.match_type === 'practice'
    ? (match.end_time ? match.player_one.games_won : match.player_one.games_won + 1)
    : null
  const score = match.match_type === 'practice'
    ? `${rackCount} rack${rackCount !== 1 ? 's' : ''}`
    : `${match.player_one.games_won} - ${match.player_two.games_won}`
  const isActive = !match.end_time
  const hasStream = !!camera

  return (
    <Box sx={{ p: 2 }}>
      <Button startIcon={<ArrowBackIcon />} onClick={() => navigate('/')} sx={{ mb: 2 }}>
        Back to Home
      </Button>
      {downloadError && (
        <Alert severity="error" sx={{ mb: 2 }} onClose={() => setDownloadError('')}>
          {downloadError}
        </Alert>
      )}
      <Paper sx={{ p: 3 }}>
        <Box display="flex" alignItems="center" gap={2} sx={{ mb: 2, flexWrap: 'wrap' }}>
          <Typography variant="h4" component="h1">
            {formatMatchTitle(match)}
          </Typography>
          <Typography variant="h5" component="span" color="primary" fontWeight={600}>
            {score}
          </Typography>
          {isActive && <Chip label="In progress" color="primary" size="small" />}
          {match.can_edit && isActive && (
            <Button
              startIcon={<EditIcon />}
              variant="outlined"
              size="small"
              onClick={openEditDialog}
            >
              Edit details
            </Button>
          )}
          {match.end_time && (() => {
            const endedLabel = formatMatchWinner(match)
            return endedLabel ? (
              <Chip
                label={endedLabel}
                color="default"
                size="small"
              />
            ) : null
          })()}
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
                setStreamUrl(`/api/cameras/${camera.id}/stream`)
              }}
              onStreamError={() => setStreamError(true)}
              rtmpActive={rtmpActive}
              cameraName={camera.name}
              locationName={locationName}
              overlayMatch={null}
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
          ) : (() => {
            const endedMessage = formatMatchWinner(match)
            return endedMessage ? (
              <Typography color="text.secondary" variant="body2">
                {endedMessage}
              </Typography>
            ) : null
          })()}
        </Box>

        {match.score_history?.length > 0 && (
          <MatchHistory match={match} recordDeleteAfter={recordDeleteAfter} onError={(err) => setDownloadError(err.message || 'Download failed')} />
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

      <Dialog open={editDialogOpen} onClose={() => !editSaving && setEditDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Edit match details</DialogTitle>
        <DialogContent>
          {editError && (
            <Alert severity="error" sx={{ mb: 2 }} onClose={() => setEditError('')}>
              {editError}
            </Alert>
          )}
          <Stack spacing={2} sx={{ mt: 1 }}>
            <Typography variant="subtitle2" color="text.secondary">Player 1</Typography>
            <TextField
              label="Name"
              value={editForm.name1 || ''}
              onChange={(e) => setEditForm((f) => ({ ...f, name1: e.target.value }))}
              fullWidth
              size="small"
            />
            <Box display="flex" gap={1}>
              <FormControl size="small" sx={{ minWidth: 100 }}>
                <InputLabel>Rating type</InputLabel>
                <Select
                  value={editForm.rating1Type || 'Apa'}
                  label="Rating type"
                  onChange={(e) => setEditForm((f) => ({ ...f, rating1Type: e.target.value }))}
                >
                  <MenuItem value="Apa">APA</MenuItem>
                  <MenuItem value="Fargo">Fargo</MenuItem>
                </Select>
              </FormControl>
              <TextField
                label="Rating"
                type="number"
                value={editForm.rating1Value ?? ''}
                onChange={(e) => setEditForm((f) => ({ ...f, rating1Value: e.target.value }))}
                size="small"
                sx={{ width: 100 }}
                inputProps={{ min: 0 }}
              />
            </Box>
            <TextField
              label="Race to"
              type="number"
              value={editForm.raceTo1 ?? ''}
              onChange={(e) => setEditForm((f) => ({ ...f, raceTo1: Number(e.target.value) || 0 }))}
              size="small"
              sx={{ width: 100 }}
              inputProps={{ min: 0, max: 21 }}
            />
            {match.match_type === 'standard' && (
              <>
                <Typography variant="subtitle2" color="text.secondary" sx={{ mt: 1 }}>Player 2</Typography>
                <TextField
                  label="Name"
                  value={editForm.name2 || ''}
                  onChange={(e) => setEditForm((f) => ({ ...f, name2: e.target.value }))}
                  fullWidth
                  size="small"
                />
                <Box display="flex" gap={1}>
                  <FormControl size="small" sx={{ minWidth: 100 }}>
                    <InputLabel>Rating type</InputLabel>
                    <Select
                      value={editForm.rating2Type || 'Apa'}
                      label="Rating type"
                      onChange={(e) => setEditForm((f) => ({ ...f, rating2Type: e.target.value }))}
                    >
                      <MenuItem value="Apa">APA</MenuItem>
                      <MenuItem value="Fargo">Fargo</MenuItem>
                    </Select>
                  </FormControl>
                  <TextField
                    label="Rating"
                    type="number"
                    value={editForm.rating2Value ?? ''}
                    onChange={(e) => setEditForm((f) => ({ ...f, rating2Value: e.target.value }))}
                    size="small"
                    sx={{ width: 100 }}
                    inputProps={{ min: 0 }}
                  />
                </Box>
                <TextField
                  label="Race to"
                  type="number"
                  value={editForm.raceTo2 ?? ''}
                  onChange={(e) => setEditForm((f) => ({ ...f, raceTo2: Number(e.target.value) || 0 }))}
                  size="small"
                  sx={{ width: 100 }}
                  inputProps={{ min: 0, max: 21 }}
                />
              </>
            )}
            <Typography variant="subtitle2" color="text.secondary" sx={{ mt: 1 }}>Description</Typography>
            <TextField
              label="Description"
              value={editForm.description || ''}
              onChange={(e) => setEditForm((f) => ({ ...f, description: e.target.value }))}
              fullWidth
              multiline
              rows={3}
              size="small"
              placeholder="Optional description (supports newlines)"
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setEditDialogOpen(false)} disabled={editSaving}>Cancel</Button>
          <Button variant="contained" onClick={handleSaveEdit} disabled={editSaving}>
            {editSaving ? 'Saving…' : 'Save'}
          </Button>
        </DialogActions>
      </Dialog>

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
    </Box>
  )
}
