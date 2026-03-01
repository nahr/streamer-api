import { useState, useEffect } from 'react'
import { Box, Typography, Paper, Chip, TextField, Button } from '@mui/material'
import { getFacebookStatus } from '../../cameras/api/cameras.js'
import { getSettings, updateSettings } from '../api/settings.js'
import { useApiInfo } from '../../../apiInfoStore.jsx'

export function ServerSettings() {
  const { refetch } = useApiInfo()
  const [facebookConfigured, setFacebookConfigured] = useState(false)
  const [facebookRedirectUri, setFacebookRedirectUri] = useState('')
  const [loading, setLoading] = useState(true)
  const [locationName, setLocationName] = useState('')
  const [locationNameSaving, setLocationNameSaving] = useState(false)
  const [locationNameSaved, setLocationNameSaved] = useState(false)

  useEffect(() => {
    let cancelled = false
    async function check() {
      try {
        const [fbStatus, settings] = await Promise.all([
          getFacebookStatus(),
          getSettings(),
        ])
        if (!cancelled) {
          setFacebookConfigured(fbStatus.configured)
          setFacebookRedirectUri(fbStatus.redirect_uri || '')
          setLocationName(settings.location_name || '')
        }
      } catch {
        if (!cancelled) setFacebookConfigured(false)
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    check()
    return () => { cancelled = true }
  }, [])

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

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Server Settings
      </Typography>

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
          Facebook Live
        </Typography>
        {loading ? (
          <Typography color="text.secondary">Checking…</Typography>
        ) : (
          <>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
              <Typography color="text.secondary">
                Status:
              </Typography>
              <Chip
                label={facebookConfigured ? 'Configured' : 'Not configured'}
                size="small"
                color={facebookConfigured ? 'success' : 'default'}
              />
            </Box>
            {!facebookConfigured ? (
              <Typography variant="body2" color="text.secondary" component="div">
                <p>To enable &quot;Go Live with Facebook&quot;, set these environment variables:</p>
                <ul>
                  <li><code>FACEBOOK_APP_ID</code> – Your Facebook App ID from developers.facebook.com</li>
                  <li><code>FACEBOOK_APP_SECRET</code> – Your Facebook App Secret</li>
                  <li><code>BASE_URL</code> – Optional. OAuth callback URL is derived from the current host; set this only if needed (e.g. behind a proxy)</li>
                </ul>
                <p>Users will sign in with their own Facebook account when they click &quot;Go Live with Facebook&quot;. Streams go to their profile. Add <code>publish_video</code> to your app&apos;s permissions.</p>
                <p>Requirements: Account 60+ days old; 100+ followers for profile streaming.</p>
              </Typography>
            ) : (
              <Typography variant="body2" color="text.secondary" component="div">
                <p><strong>Redirect URI to add in Facebook:</strong></p>
                <p>In developers.facebook.com: <strong>Products</strong> → <strong>Facebook Login</strong> → <strong>Settings</strong> → under <strong>Client OAuth Settings</strong>, add this to <strong>Valid OAuth Redirect URIs</strong>:</p>
                <Box component="pre" sx={{ bgcolor: 'action.hover', p: 1.5, borderRadius: 1, overflow: 'auto', fontSize: '0.875rem' }}>
                  {facebookRedirectUri || '(loading…)'}
                </Box>
                <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                  The URI must match exactly. After saving, the stream will appear on your Facebook profile when you go live.
                </Typography>
              </Typography>
            )}
          </>
        )}

      </Paper>
    </Box>
  )
}
