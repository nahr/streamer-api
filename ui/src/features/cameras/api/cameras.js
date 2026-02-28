/**
 * Camera API client.
 * Camera types: Rtsp { url }, Internal, Usb { device }
 */

/**
 * @typedef {'rtsp' | 'internal' | 'usb'} CameraTypeKey
 */

/**
 * @typedef {Object} Camera
 * @property {string} id
 * @property {string} name
 * @property {{ Rtsp?: { url: string }, Internal?: null, Usb?: { device: string } }} camera_type
 */

/**
 * Build camera_type payload for API.
 * @param {CameraTypeKey} type
 * @param {string} [url] - For Rtsp
 * @param {string} [device] - For Usb
 * @returns {{ Rtsp?: { url: string }, Internal?: null, Usb?: { device: string } }}
 */
function buildCameraType(type, url = '', device = '') {
  switch (type) {
    case 'rtsp':
      return { Rtsp: { url } }
    case 'internal':
      return { Internal: null }
    case 'usb':
      return { Usb: { device } }
    default:
      return { Internal: null }
  }
}

/**
 * Parse camera_type from API response to { type, url?, device? }.
 * @param {Camera['camera_type']} cameraType
 * @returns {{ type: CameraTypeKey, url?: string, device?: string }}
 */
export function parseCameraType(cameraType) {
  if (cameraType?.Rtsp) {
    return { type: 'rtsp', url: cameraType.Rtsp.url || '' }
  }
  if (cameraType?.Internal !== undefined) {
    return { type: 'internal' }
  }
  if (cameraType?.Usb) {
    return { type: 'usb', device: cameraType.Usb.device || '' }
  }
  return { type: 'internal' }
}

/**
 * @returns {Promise<Camera[]>}
 */
export async function listCameras() {
  const res = await fetch('/api/cameras')
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
  const res = await fetch(`/api/cameras/${id}`)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch camera')
  }
  return res.json()
}

/**
 * @param {string} name
 * @param {CameraTypeKey} type
 * @param {string} [url]
 * @param {string} [device]
 * @returns {Promise<{ id: string }>}
 */
export async function createCamera(name, type, url = '', device = '') {
  const res = await fetch('/api/cameras', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      camera_type: buildCameraType(type, url, device),
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
 * @param {CameraTypeKey} type
 * @param {string} [url]
 * @param {string} [device]
 * @returns {Promise<void>}
 */
export async function updateCamera(id, name, type, url = '', device = '') {
  const res = await fetch(`/api/cameras/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      camera_type: buildCameraType(type, url, device),
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
  const res = await fetch(`/api/cameras/${id}`, { method: 'DELETE' })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to delete camera')
  }
}
