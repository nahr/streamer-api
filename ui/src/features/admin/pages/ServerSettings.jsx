import { useState, useEffect } from 'react'
import { Box, Typography, Paper, Chip, TextField, Button, CircularProgress } from '@mui/material'
import { getSettings, updateSettings } from '../api/settings.js'
import { checkForUpgrades, upgradeNow } from '../api/upgrade.js'
import { useApiInfo } from '../../../apiInfoStore.jsx'

export function ServerSettings() {
  const { refetch, version, candidateVersion } = useApiInfo()
  const [loading, setLoading] = useState(true)
  const [locationName, setLocationName] = useState('')
  const [locationNameSaving, setLocationNameSaving] = useState(false)
  const [locationNameSaved, setLocationNameSaved] = useState(false)
  const [recordPath, setRecordPath] = useState('')
  const [recordSegmentDuration, setRecordSegmentDuration] = useState('1m')
  const [recordDeleteAfter, setRecordDeleteAfter] = useState('24h')
  const [rollingSaving, setRollingSaving] = useState(false)
  const [rollingSaved, setRollingSaved] = useState(false)
  const [upgradeOutput, setUpgradeOutput] = useState('')
  const [upgradeRunning, setUpgradeRunning] = useState(false)
  const [upgradePhase, setUpgradePhase] = useState(null) // 'check' | 'upgrade' | null

  useEffect(() => {
    let cancelled = false
    async function check() {
      try {
        const settings = await getSettings()
        if (!cancelled) {
          setLocationName(settings.location_name || '')
          setRecordPath(settings.record_path || '')
          setRecordSegmentDuration(settings.record_segment_duration || '1m')
          setRecordDeleteAfter(settings.record_delete_after || '24h')
        }
      } catch {
        // ignore
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    check()
    return () => { cancelled = true }
  }, [])

  const handleSaveRollingStorage = async () => {
    setRollingSaving(true)
    setRollingSaved(false)
    try {
      await updateSettings({
        record_path: recordPath,
        record_segment_duration: recordSegmentDuration,
        record_delete_after: recordDeleteAfter,
      })
      setRollingSaved(true)
      setTimeout(() => setRollingSaved(false), 2000)
    } catch {
      // Error could be shown via snackbar
    } finally {
      setRollingSaving(false)
    }
  }

  const handleSaveLocationName = async () => {
    setLocationNameSaving(true)
    setLocationNameSaved(false)
    try {
      await updateSettings({ location_name: locationName })
      setLocationNameSaved(true)
      setTimeout(() => setLocationNameSaved(false), 2000)
      refetch({ silent: true })
    } catch {
      // Error could be shown via snackbar; for now we just stop loading
    } finally {
      setLocationNameSaving(false)
    }
  }

  const upToDate = version === candidateVersion

  const handleCheckForUpgrades = async () => {
    setUpgradeOutput('')
    setUpgradeRunning(true)
    setUpgradePhase('check')
    try {
      await checkForUpgrades(() => {})
      refetch({ silent: true })
    } catch (e) {
      setUpgradeOutput(`Error: ${e.message}`)
    } finally {
      setUpgradeRunning(false)
      setUpgradePhase(null)
    }
  }

  const handleUpgradeNow = async () => {
    setUpgradeOutput('')
    setUpgradeRunning(true)
    setUpgradePhase('upgrade')
    try {
      await upgradeNow((chunk) => setUpgradeOutput((prev) => prev + chunk))
      refetch({ silent: true })
    } catch (e) {
      setUpgradeOutput((prev) => prev + (prev ? '\n' : '') + `Error: ${e.message}`)
    } finally {
      setUpgradeRunning(false)
      setUpgradePhase(null)
    }
  }

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Server Settings
      </Typography>

      <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
        <Typography variant="h6" gutterBottom>
          Server Version
        </Typography>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, flexWrap: 'wrap', mb: 2 }}>
          <Typography color="text.secondary">Current version:</Typography>
          <Typography sx={{ fontFamily: 'monospace' }}>{version || '(loading…)'}</Typography>
          <Chip label={upToDate ? 'Up to date' : (candidateVersion ? `Update available (${candidateVersion})` : 'Update available')} size="small" color={upToDate ? 'success' : 'warning'} />
        </Box>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
          <Button
            variant="outlined"
            size="small"
            onClick={handleCheckForUpgrades}
            disabled={upgradeRunning}
          >
            Check for upgrades
          </Button>
          <Button
            variant="contained"
            size="small"
            onClick={handleUpgradeNow}
            disabled={upgradeRunning || upToDate}
          >
            Upgrade now
          </Button>
          {upgradePhase === 'check' && (
            <CircularProgress size={24} sx={{ ml: 1 }} />
          )}
        </Box>
        {upgradePhase !== 'check' && upgradeOutput && (
          <Box
            component="pre"
            sx={{
              bgcolor: 'action.hover',
              p: 1.5,
              borderRadius: 1,
              overflow: 'auto',
              fontSize: '0.75rem',
              fontFamily: 'monospace',
              maxHeight: 300,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
            }}
          >
            {upgradeOutput}
          </Box>
        )}
      </Paper>

      <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
        <Typography variant="h6" gutterBottom>
          Location Name
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          A display name for this Table TV installation (e.g. venue or room name).
        </Typography>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, flexWrap: 'wrap' }}>
          <TextField
            label="Location name"
            value={locationName}
            onChange={(e) => setLocationName(e.target.value)}
            placeholder="e.g. Main Pool Hall"
            size="small"
            sx={{ minWidth: 240 }}
          />
          <Button
            variant="contained"
            onClick={handleSaveLocationName}
            disabled={locationNameSaving}
          >
            {locationNameSaving ? 'Saving…' : locationNameSaved ? 'Saved' : 'Save'}
          </Button>
        </Box>
      </Paper>

      <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
        <Typography variant="h6" gutterBottom>
          Rolling Video Storage (MediaMTX)
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          Configure rolling recording for all cameras. Recordings are stored in segments and automatically deleted after the retention period. Leave path empty for default location.
        </Typography>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, maxWidth: 400 }}>
          <TextField
            label="Record path"
            value={recordPath}
            onChange={(e) => setRecordPath(e.target.value)}
            placeholder="e.g. /app/data/recordings (empty = default)"
            size="small"
          />
          <TextField
            label="Segment duration"
            value={recordSegmentDuration}
            onChange={(e) => setRecordSegmentDuration(e.target.value)}
            placeholder="e.g. 1m, 30m, 1h"
            size="small"
            helperText="Duration per segment file"
          />
          <TextField
            label="Delete after"
            value={recordDeleteAfter}
            onChange={(e) => setRecordDeleteAfter(e.target.value)}
            placeholder="e.g. 24h, 7d (empty = keep forever)"
            size="small"
            helperText="Auto-delete recordings after this period"
          />
          <Button
            variant="contained"
            onClick={handleSaveRollingStorage}
            disabled={rollingSaving}
          >
            {rollingSaving ? 'Saving…' : rollingSaved ? 'Saved' : 'Save'}
          </Button>
        </Box>
      </Paper>
    </Box>
  )
}
