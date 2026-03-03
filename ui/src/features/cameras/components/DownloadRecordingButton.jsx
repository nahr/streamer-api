import { useState } from 'react'
import { Button, Menu, MenuItem } from '@mui/material'
import DownloadIcon from '@mui/icons-material/Download'
import ArrowDropDownIcon from '@mui/icons-material/ArrowDropDown'
import PhotoLibraryIcon from '@mui/icons-material/PhotoLibrary'
import FolderIcon from '@mui/icons-material/Folder'
import { downloadGameRecording, shareGameRecording, canShareVideo } from '../api/poolMatches.js'

/**
 * Button to download a game recording. On mobile (when Web Share API supports files),
 * shows a menu with "Save to Photos" and "Save to Files". Both use the share sheet on
 * mobile since Safari ignores the download attribute for blob URLs.
 *
 * @param {Object} props
 * @param {string} props.cameraId
 * @param {number | (() => number)} props.startMs - Start time in ms, or function to compute at click time
 * @param {number} props.durationSec
 * @param {string} props.filename
 * @param {boolean} [props.disabled]
 * @param {() => void} [props.onLoadingStart]
 * @param {() => void} [props.onLoadingEnd]
 * @param {(err: Error) => void} [props.onError] - Called when download/share fails
 * @param {string} [props.label] - Button label, default "Download"
 * @param {'text'|'outlined'|'contained'} [props.variant] - Button variant
 * @param {Object} [props.sx] - MUI sx prop for the button
 */
export function DownloadRecordingButton({
  cameraId,
  startMs,
  durationSec,
  filename,
  disabled = false,
  onLoadingStart,
  onLoadingEnd,
  onError,
  label = 'Download',
  variant,
  sx,
}) {
  const [anchorEl, setAnchorEl] = useState(null)
  const [loading, setLoading] = useState(false)
  const showMenu = canShareVideo()

  const getStartMs = () => (typeof startMs === 'function' ? startMs() : startMs)

  const runAction = async (fn) => {
    setLoading(true)
    onLoadingStart?.()
    try {
      await fn()
    } catch (err) {
      // Don't surface AbortError - user cancelled the share sheet
      if (err?.name === 'AbortError') return
      console.error('Download/share failed', err)
      onError?.(err)
    } finally {
      setLoading(false)
      onLoadingEnd?.()
      setAnchorEl(null)
    }
  }

  // On mobile, both options use share (Safari ignores download attribute for blob URLs)
  const handleShare = () => runAction(() => shareGameRecording(cameraId, getStartMs(), durationSec, filename))
  const handleDownload = showMenu
    ? handleShare
    : () => runAction(() => downloadGameRecording(cameraId, getStartMs(), durationSec, filename))

  const isDisabled = disabled || loading

  if (showMenu) {
    return (
      <>
        <Button
          size="small"
          variant={variant}
          startIcon={<DownloadIcon />}
          endIcon={<ArrowDropDownIcon />}
          onClick={(e) => setAnchorEl(e.currentTarget)}
          disabled={isDisabled}
          sx={sx}
        >
          {loading ? 'Downloading…' : label}
        </Button>
        <Menu
          anchorEl={anchorEl}
          open={Boolean(anchorEl)}
          onClose={() => setAnchorEl(null)}
          anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
          transformOrigin={{ vertical: 'top', horizontal: 'right' }}
        >
          <MenuItem
            onClick={handleShare}
            disabled={loading}
          >
            <PhotoLibraryIcon fontSize="small" sx={{ mr: 1 }} />
            Save to Photos
          </MenuItem>
          <MenuItem
            onClick={handleDownload}
            disabled={loading}
          >
            <FolderIcon fontSize="small" sx={{ mr: 1 }} />
            Save to Files
          </MenuItem>
        </Menu>
      </>
    )
  }

  return (
    <Button
      size="small"
      variant={variant}
      startIcon={<DownloadIcon />}
      onClick={handleDownload}
      disabled={isDisabled}
      sx={sx}
    >
      {loading ? 'Downloading…' : label}
    </Button>
  )
}
