/**
 * Camera API client.
 * Camera type: Rtsp { url }
 */
import { fetchWithAuth } from '../../../apiClient.js'

/**
 * @typedef {Object} Camera
 * @property {string} id
 * @property {string} name
 * @property {{ Rtsp: { url: string } }} camera_type
 */

/**
 * Build camera_type payload for API.
 * @param {string} [url]
 * @returns {{ Rtsp: { url: string } }}
 */
function buildCameraType(url = '') {
  return { Rtsp: { url } }
}

/**
 * Format camera type for display.
 * @param {Camera['camera_type']} cameraType
 * @param {'label'|'labelNoUrl'|'string'|'stringNoUrl'} [format='string'] - label: { label, detail }; labelNoUrl: { label, detail: null }; string: "RTSP: url"; stringNoUrl: "RTSP"
 * @returns {{ label: string, detail: string|null }|string}
 */
export function formatCameraType(cameraType, format = 'string') {
  const parsed = parseCameraType(cameraType)
  const url = parsed.url || '(no url)'
  if (format === 'label') return { label: 'RTSP', detail: url }
  if (format === 'labelNoUrl') return { label: 'RTSP', detail: null }
  if (format === 'stringNoUrl') return 'RTSP'
  return `RTSP: ${url}`
}

/**
 * Parse camera_type from API response to { type, url }.
 * @param {Camera['camera_type']} cameraType
 * @returns {{ type: 'rtsp', url: string }}
 */
export function parseCameraType(cameraType) {
  if (cameraType?.Rtsp) {
    return { type: 'rtsp', url: cameraType.Rtsp.url || '' }
  }
  return { type: 'rtsp', url: '' }
}

/**
 * @returns {Promise<Camera[]>}
 */
export async function listCameras() {
  const res = await fetchWithAuth('/api/cameras')
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to list cameras')
  }
  return res.json()
}

/**
 * @param {string} id
 * @returns {Promise<Camera>}
 */
export async function getCamera(id) {
  const res = await fetchWithAuth(`/api/cameras/${id}`)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch camera')
  }
  return res.json()
}

/**
 * @param {string} name
 * @param {string} [url]
 * @returns {Promise<{ id: string }>}
 */
export async function createCamera(name, url = '') {
  const res = await fetchWithAuth('/api/cameras', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      camera_type: buildCameraType(url),
    }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to create camera')
  }
  return res.json()
}

/**
 * @param {string} id
 * @param {string} name
 * @param {string} [url]
 * @returns {Promise<void>}
 */
export async function updateCamera(id, name, url = '') {
  const res = await fetchWithAuth(`/api/cameras/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      camera_type: buildCameraType(url),
    }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to update camera')
  }
}

/**
 * @param {string} id
 * @returns {Promise<void>}
 */
export async function deleteCamera(id) {
  const res = await fetchWithAuth(`/api/cameras/${id}`, { method: 'DELETE' })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to delete camera')
  }
}

/**
 * Check if RTMP stream is active for a camera.
 * Only works for RTSP cameras. Requires FFmpeg on the server.
 * @param {string} cameraId
 * @param {string} rtmpUrl - e.g. rtmp://a.rtmp.youtube.com/live2/xxxx or rtmps://...
 * @returns {Promise<{ ok: boolean }>}
 */
export async function startRtmpStream(cameraId, rtmpUrl) {
  const res = await fetchWithAuth(`/api/cameras/${cameraId}/stream/rtmp`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url: rtmpUrl }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to start RTMP stream')
  }
  return res.json()
}

/**
 * Stop the RTMP stream for a camera.
 * @param {string} cameraId
 * @returns {Promise<{ ok: boolean }>}
 */
export async function stopRtmpStream(cameraId) {
  const res = await fetchWithAuth(`/api/cameras/${cameraId}/stream/rtmp/stop`, {
    method: 'POST',
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to stop RTMP stream')
  }
  return res.json()
}

/**
 * Fetch available recording segments for a camera in the given time range.
 * @param {string} cameraId
 * @param {number} startMs - Start time in milliseconds since epoch
 * @param {number} endMs - End time in milliseconds since epoch
 * @returns {Promise<{ start_ms: number, duration_sec: number }[]>}
 */
export async function getRecordingTimeline(cameraId, startMs, endMs) {
  const res = await fetchWithAuth(
    `/api/cameras/${encodeURIComponent(cameraId)}/recordings/timeline?start=${startMs}&end=${endMs}`
  )
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch recording timeline')
  }
  return res.json()
}

/**
 * Check if RTMP stream is active for a camera.
 * @param {string} cameraId
 * @returns {Promise<{ active: boolean }>}
 */
export async function getRtmpStreamStatus(cameraId) {
  const res = await fetchWithAuth(`/api/cameras/${cameraId}/stream/rtmp/status`)
  if (!res.ok) throw new Error('Failed to get RTMP status')
  return res.json()
}
