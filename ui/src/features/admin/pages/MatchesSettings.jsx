import { useState, useEffect } from 'react'
import {
  Box,
  Typography,
  Button,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  IconButton,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Alert,
  CircularProgress,
  Chip,
} from '@mui/material'
import DeleteIcon from '@mui/icons-material/Delete'
import { listMatches, deleteMatch } from '../../cameras/api/poolMatches.js'
import { formatTime, formatDuration, formatMatchWinner } from '../../../utils/format.js'

export function MatchesSettings() {
  const [matches, setMatches] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [deleteDialog, setDeleteDialog] = useState({ open: false, match: null })
  const [submitLoading, setSubmitLoading] = useState(false)

  const fetchMatches = async () => {
    setLoading(true)
    setError('')
    try {
      const data = await listMatches()
      setMatches([...data].sort((a, b) => b.start_time - a.start_time))
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchMatches()
  }, [])

  const handleDeleteClick = (match) => {
    setDeleteDialog({ open: true, match })
  }

  const handleDeleteConfirm = async () => {
    if (!deleteDialog.match) return
    setSubmitLoading(true)
    setError('')
    try {
      await deleteMatch(deleteDialog.match.id)
      setDeleteDialog({ open: false, match: null })
      await fetchMatches()
    } catch (err) {
      setError(err.message)
    } finally {
      setSubmitLoading(false)
    }
  }

  return (
    <Box sx={{ p: 2 }}>
      <Box display="flex" justifyContent="space-between" alignItems="center" sx={{ mb: 2 }}>
        <Typography variant="h4" component="h1" gutterBottom>
          Matches
        </Typography>
      </Box>

      {error && (
        <Alert severity="error" onClose={() => setError('')} sx={{ mb: 2 }}>
          {error}
        </Alert>
      )}

      {loading ? (
        <Box display="flex" justifyContent="center" py={4}>
          <CircularProgress />
        </Box>
      ) : (
        <TableContainer component={Paper}>
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>Match</TableCell>
                <TableCell>Score</TableCell>
                <TableCell>Camera</TableCell>
                <TableCell>Started</TableCell>
                <TableCell>Started by</TableCell>
                <TableCell>Duration</TableCell>
                <TableCell>Status</TableCell>
                <TableCell align="right">Actions</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {matches.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={8} align="center" sx={{ py: 4 }}>
                    <Typography color="text.secondary">
                      No matches yet. Start a match from a camera view.
                    </Typography>
                  </TableCell>
                </TableRow>
              ) : (
                matches.map((match) => {
                  const score = `${match.player_one.games_won} - ${match.player_two.games_won}`
                  const durationMs = (match.end_time ?? Date.now()) - match.start_time
                  return (
                    <TableRow key={match.id}>
                      <TableCell>
                        {match.player_one.name} vs {match.player_two.name}
                      </TableCell>
                      <TableCell>{score}</TableCell>
                      <TableCell>{match.camera_name}</TableCell>
                      <TableCell>{formatTime(match.start_time, 'full')}</TableCell>
                      <TableCell>{match.started_by ?? '—'}</TableCell>
                      <TableCell>{formatDuration(durationMs, { includeSeconds: false })}</TableCell>
                      <TableCell>
                        {match.end_time ? (
                          <Chip
                            label={formatMatchWinner(match)}
                            size="small"
                            color="default"
                          />
                        ) : (
                          <Chip label="Ongoing" size="small" color="primary" />
                        )}
                      </TableCell>
                      <TableCell align="right">
                        <IconButton
                          size="small"
                          color="error"
                          onClick={() => handleDeleteClick(match)}
                          aria-label="Delete"
                        >
                          <DeleteIcon />
                        </IconButton>
                      </TableCell>
                    </TableRow>
                  )
                })
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}

      <Dialog
        open={deleteDialog.open}
        onClose={() => setDeleteDialog({ open: false, match: null })}
      >
        <DialogTitle>Delete Match</DialogTitle>
        <DialogContent>
          {deleteDialog.match && (
            <Typography>
              Are you sure you want to delete the match between &quot;
              {deleteDialog.match.player_one.name}&quot; and &quot;
              {deleteDialog.match.player_two.name}&quot;? This cannot be undone.
            </Typography>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setDeleteDialog({ open: false, match: null })}>
            Cancel
          </Button>
          <Button
            color="error"
            variant="contained"
            onClick={handleDeleteConfirm}
            disabled={submitLoading}
          >
            {submitLoading ? 'Deleting...' : 'Delete'}
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  )
}
