import {
  Box,
  Typography,
  Button,
  CircularProgress,
} from '@mui/material'
import { LiveTimestamp } from '../../../components/LiveTimestamp.jsx'

/**
 * Shared stream preview with overlays (location, timestamp, score bar).
 * @param {Object} props
 * @param {string} props.streamUrl
 * @param {boolean} props.streamError
 * @param {boolean} props.previewLoaded
 * @param {(loaded: boolean) => void} props.setPreviewLoaded
 * @param {() => void} props.onRetry
 * @param {() => void} props.onStreamError - Called when img fails to load
 * @param {boolean} props.rtmpActive
 * @param {string} props.cameraName
 * @param {string} [props.locationName]
 * @param {Object|null} [props.overlayMatch] - Match to show in score overlay (when !end_time)
 */
export function StreamPreview({
  streamUrl,
  streamError,
  previewLoaded,
  setPreviewLoaded,
  onRetry,
  onStreamError,
  rtmpActive,
  cameraName,
  locationName,
  overlayMatch,
}) {
  const showScoreOverlay = overlayMatch && !overlayMatch.end_time
  const match = overlayMatch

  return (
    <Box sx={{ position: 'relative', display: 'inline-block' }}>
      {streamError ? (
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
            Check that the RTSP URL is valid and reachable
          </Typography>
          <Button
            size="small"
            variant="outlined"
            onClick={() => {
              setPreviewLoaded(false)
              onRetry()
            }}
            sx={{ mt: 1 }}
          >
            Retry
          </Button>
        </Box>
      ) : rtmpActive ? (
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
          <Typography variant="body2">Stream is live</Typography>
          <Typography variant="caption" sx={{ opacity: 0.8 }}>
            Preview paused while broadcasting
          </Typography>
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
        <Box sx={{ maxWidth: 640, width: '100%' }}>
          <Box sx={{ position: 'relative', display: 'inline-block', width: '100%' }}>
            <img
              src={streamUrl}
              alt={`${cameraName} live stream`}
              onLoad={() => setPreviewLoaded(true)}
              onError={() => { setPreviewLoaded(false); onStreamError() }}
              style={{
                width: '100%',
                borderRadius: showScoreOverlay ? '8px 8px 0 0' : 8,
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
                    {cameraName}
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
              </>
            )}
          </Box>
          {showScoreOverlay && match && (
            <Box
              sx={{
                background: 'rgba(0,0,0,0.9)',
                color: '#fff',
                py: 1.25,
                px: 2,
                borderRadius: '0 0 8px 8px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: match.match_type === 'practice' ? 'flex-start' : 'space-between',
                gap: 2,
              }}
            >
              {match.match_type === 'practice' ? (
                <>
                  <Typography variant="subtitle2" fontWeight={600} noWrap sx={{ maxWidth: '100%' }}>
                    Practice: {match.player_one.name}
                  </Typography>
                  <Typography variant="subtitle1" fontWeight={700} sx={{ fontVariantNumeric: 'tabular-nums' }}>
                    Rack #{match.player_one.games_won}
                  </Typography>
                </>
              ) : (
                <>
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
                        {match.player_one.games_won}
                      </Typography>
                    </Box>
                    <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center' }}>
                      <Typography variant="caption" color="rgba(255,255,255,0.8)">
                        race to
                      </Typography>
                      <Typography variant="caption" color="rgba(255,255,255,0.8)">
                        {match.player_one.race_to}/{match.player_two.race_to}
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
                        {match.player_two.games_won}
                      </Typography>
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
                </>
              )}
            </Box>
              )}
        </Box>
      )}
    </Box>
  )
}
