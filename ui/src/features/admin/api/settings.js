/**
 * Settings API client.
 * @returns {Promise<{ location_name: string }>}
 */
export async function getSettings() {
  const res = await fetch('/api/settings')
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch settings')
  }
  return res.json()
}

/**
 * Update settings.
 * @param {{ location_name?: string }} settings
 * @returns {Promise<{ location_name: string }>}
 */
export async function updateSettings(settings) {
  const res = await fetch('/api/settings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(settings),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to update settings')
  }
  return res.json()
}
