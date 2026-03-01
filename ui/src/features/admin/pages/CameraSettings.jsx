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
  TextField,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Alert,
  CircularProgress,
} from '@mui/material'
import EditIcon from '@mui/icons-material/Edit'
import DeleteIcon from '@mui/icons-material/Delete'
import AddIcon from '@mui/icons-material/Add'
import {
  listCameras,
  createCamera,
  updateCamera,
  deleteCamera,
  parseCameraType,
  formatCameraType,
} from '../../../features/cameras/api/cameras.js'

const CAMERA_TYPES = [
  { value: 'rtsp', label: 'RTSP' },
  { value: 'internal', label: 'Internal' },
  { value: 'usb', label: 'USB' },
]

function CameraForm({ name, type, url, device, onChange }) {
  return (
    <Box display="flex" flexDirection="column" gap={2} sx={{ mt: 1 }}>
      <TextField
        label="Name"
        value={name}
        onChange={(e) => onChange({ name: e.target.value })}
        required
        fullWidth
      />
      <FormControl fullWidth>
        <InputLabel>Type</InputLabel>
        <Select
          value={type}
          label="Type"
          onChange={(e) => onChange({ type: e.target.value })}
        >
          {CAMERA_TYPES.map((t) => (
            <MenuItem key={t.value} value={t.value}>
              {t.label}
            </MenuItem>
          ))}
        </Select>
      </FormControl>
      {type === 'rtsp' && (
        <TextField
          label="RTSP URL"
          placeholder="rtsp://..."
          value={url}
          onChange={(e) => onChange({ url: e.target.value })}
          fullWidth
        />
      )}
      {type === 'usb' && (
        <TextField
          label="Device"
          placeholder="/dev/video0"
          value={device}
          onChange={(e) => onChange({ device: e.target.value })}
          fullWidth
        />
      )}
    </Box>
  )
}

export function CameraSettings() {
  const [cameras, setCameras] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingId, setEditingId] = useState(null)
  const [form, setForm] = useState({
    name: '',
    type: 'internal',
    url: '',
    device: '',
  })
  const [submitLoading, setSubmitLoading] = useState(false)
  const [deleteDialog, setDeleteDialog] = useState({ open: false, camera: null })

  const fetchCameras = async () => {
    setLoading(true)
    setError('')
    try {
      const data = await listCameras()
      setCameras(data)
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchCameras()
  }, [])

  const openAddDialog = () => {
    setEditingId(null)
    setForm({ name: '', type: 'internal', url: '', device: '' })
    setDialogOpen(true)
  }

  const openEditDialog = (camera) => {
    const parsed = parseCameraType(camera.camera_type)
    setEditingId(camera.id)
    setForm({
      name: camera.name,
      type: parsed.type,
      url: parsed.url ?? '',
      device: parsed.device ?? '',
    })
    setDialogOpen(true)
  }

  const handleFormChange = (updates) => {
    setForm((prev) => ({ ...prev, ...updates }))
  }

  const handleSubmit = async (e) => {
    e.preventDefault()
    if (!form.name.trim()) return
    setSubmitLoading(true)
    setError('')
    try {
      if (editingId) {
        await updateCamera(
          editingId,
          form.name.trim(),
          form.type,
          form.url,
          form.device
        )
      } else {
        await createCamera(
          form.name.trim(),
          form.type,
          form.url,
          form.device
        )
      }
      setDialogOpen(false)
      await fetchCameras()
    } catch (err) {
      setError(err.message)
    } finally {
      setSubmitLoading(false)
    }
  }

  const handleDeleteClick = (camera) => {
    setDeleteDialog({ open: true, camera })
  }

  const handleDeleteConfirm = async () => {
    if (!deleteDialog.camera) return
    setSubmitLoading(true)
    setError('')
    try {
      await deleteCamera(deleteDialog.camera.id)
      setDeleteDialog({ open: false, camera: null })
      await fetchCameras()
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
          Camera Settings
        </Typography>
        <Button
          variant="contained"
          startIcon={<AddIcon />}
          onClick={openAddDialog}
        >
          Add Camera
        </Button>
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
                <TableCell>Name</TableCell>
                <TableCell>Type</TableCell>
                <TableCell align="right">Actions</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {cameras.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} align="center" sx={{ py: 4 }}>
                    <Typography color="text.secondary">
                      No cameras configured. Click &quot;Add Camera&quot; to add one.
                    </Typography>
                  </TableCell>
                </TableRow>
              ) : (
                cameras.map((camera) => (
                  <TableRow key={camera.id}>
                    <TableCell>{camera.name}</TableCell>
                    <TableCell>{formatCameraType(camera.camera_type)}</TableCell>
                    <TableCell align="right">
                      <IconButton
                        size="small"
                        onClick={() => openEditDialog(camera)}
                        aria-label="Edit"
                      >
                        <EditIcon />
                      </IconButton>
                      <IconButton
                        size="small"
                        color="error"
                        onClick={() => handleDeleteClick(camera)}
                        aria-label="Delete"
                      >
                        <DeleteIcon />
                      </IconButton>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}

      <Dialog open={dialogOpen} onClose={() => setDialogOpen(false)} maxWidth="sm" fullWidth>
        <form onSubmit={handleSubmit}>
          <DialogTitle>
            {editingId ? 'Edit Camera' : 'Add Camera'}
          </DialogTitle>
          <DialogContent>
            <CameraForm
              name={form.name}
              type={form.type}
              url={form.url}
              device={form.device}
              onChange={handleFormChange}
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setDialogOpen(false)}>Cancel</Button>
            <Button type="submit" variant="contained" disabled={submitLoading || !form.name.trim()}>
              {submitLoading ? 'Saving...' : editingId ? 'Save' : 'Add'}
            </Button>
          </DialogActions>
        </form>
      </Dialog>

      <Dialog
        open={deleteDialog.open}
        onClose={() => setDeleteDialog({ open: false, camera: null })}
      >
        <DialogTitle>Delete Camera</DialogTitle>
        <DialogContent>
          {deleteDialog.camera && (
            <Typography>
              Are you sure you want to delete &quot;{deleteDialog.camera.name}&quot;?
            </Typography>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setDeleteDialog({ open: false, camera: null })}>
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
