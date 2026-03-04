import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Box, Typography, Stack, Button } from '@mui/material'
import HistoryIcon from '@mui/icons-material/History'
import { DownloadRecordingButton } from './DownloadRecordingButton.jsx'
import { formatTime, formatDuration, formatMatchTitle, isRecordingAvailable, formatRecordingFilename } from '../../../utils/format.js'

export function MatchHistory({ matches, match, recordDeleteAfter, onError }) {
  const navigate = useNavigate()
  const [downloadingGame, setDownloadingGame] = useState(null)

  if (!matches && !match) return null

  // Normalize to an array of matches for the "camera" view
  const list = matches || [match]
  const single = !!match && !matches

  return (
    <Box sx={{ mt: single ? 2 : 3, pt: 2, borderTop: 1, borderColor: 'divider' }}>
      <Typography variant="h6" sx={{ mb: 1.5, display: 'flex', alignItems: 'center', gap: 1 }}>
        <HistoryIcon fontSize="small" />
        {single ? 'Score history' : 'Practice & match history'}
      </Typography>
      <Stack spacing={single ? 0 : 3} component={single ? 'ul' : undefined} sx={single ? { listStyle: 'none', pl: 0, m: 0 } : undefined}>
        {list.map((m) => {
          const entries = m.score_history || []
          if ((entries.length ?? 0) === 0) return null
          return (
            <Box key={m.id || 'single'} component={single ? 'li' : 'div'}>
              {!single && (
                <>
                  <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 1 }}>
                    {formatMatchTitle(m)}
                    <Typography component="span" variant="body2" color="text.secondary" sx={{ ml: 1 }}>
                      {formatTime(m.start_time, 'full')}
                      {m.end_time && ` – ${formatTime(m.end_time, 'full')}`}
                    </Typography>
                  </Typography>
                  <Button size="small" sx={{ mb: 1 }} onClick={() => navigate(`/match/${m.id}`)}>
                    View match
                  </Button>
                </>
              )}

              <Stack component="ul" spacing={0} sx={{ listStyle: 'none', pl: 0, m: 0 }}>
                {entries.map((entry, i) => {
                  const prev = i > 0 ? entries[i - 1] : { player_one_games_won: 0, player_two_games_won: 0 }
                  const p1Increased = entry.player_one_games_won > prev.player_one_games_won
                  const player = p1Increased ? m.player_one.name : m.player_two.name
                  const gameNumber = entry.player_one_games_won + entry.player_two_games_won
                  const rackNumber = entry.player_one_games_won
                  const startMs = i === 0 ? m.start_time : prev.timestamp
                  const downloadStartMs = m.match_type === 'practice' && i > 0 ? prev.timestamp + 2000 : startMs
                  const durationSec = Math.max(1, (entry.timestamp - downloadStartMs) / 1000)
                  const downloadKey = `${m.id}-${i}`
                  const isDownloading = downloadingGame === downloadKey
                  return (
                    <Box
                      key={i}
                      component="li"
                      sx={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 1,
                        py: 1,
                        borderBottom: i < entries.length - 1 ? 1 : 0,
                        borderColor: 'divider',
                      }}
                    >
                      <Typography variant="body2" color="text.secondary">
                        {formatTime(startMs, 'withSeconds')} – {formatTime(entry.timestamp, 'withSeconds')}
                        {' · '}
                        {formatDuration(entry.timestamp - startMs)}
                      </Typography>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        <Typography variant="body1" sx={{ flex: 1 }}>
                          {m.match_type === 'practice'
                            ? `Rack ${rackNumber}`
                            : `${player} won game ${gameNumber}, ${entry.player_one_games_won} – ${entry.player_two_games_won}`}
                        </Typography>
                        {m.camera_id && isRecordingAvailable(entry.timestamp, recordDeleteAfter) && (
                          <DownloadRecordingButton
                            cameraId={m.camera_id}
                            startMs={downloadStartMs}
                            durationSec={durationSec}
                            filename={formatRecordingFilename(startMs, m.match_type, m.match_type === 'practice' ? rackNumber : gameNumber)}
                            disabled={isDownloading}
                            onLoadingStart={() => setDownloadingGame(downloadKey)}
                            onLoadingEnd={() => setDownloadingGame(null)}
                            onError={(err) => onError && onError(err)}
                          />
                        )}
                      </Box>
                    </Box>
                  )
                })}
              </Stack>
            </Box>
          )
        })}
      </Stack>
    </Box>
  )
}

export default MatchHistory
