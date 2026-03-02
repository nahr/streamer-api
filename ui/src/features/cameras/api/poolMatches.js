/**
 * Pool match API client.
 */
import { fetchWithAuth } from '../../../apiClient.js'

/**
 * @typedef {Object} MatchPlayer
 * @property {string} name
 * @property {number} race_to
 * @property {number} games_won
 * @property {{ type: string, value: number } | null} [rating]
 */

/**
 * @typedef {Object} ScoreHistoryEntry
 * @property {number} player_one_games_won
 * @property {number} player_two_games_won
 * @property {number} timestamp - Unix timestamp in milliseconds
 */

/**
 * @typedef {Object} PoolMatch
 * @property {string} id
 * @property {MatchPlayer} player_one
 * @property {MatchPlayer} player_two
 * @property {number} start_time
 * @property {number | null} [end_time]
 * @property {string} camera_id
 * @property {string} camera_name
 * @property {string} [started_by] - Display name of user who started the match
 * @property {string} [description] - Match description (supports newlines), used in live video post
 * @property {ScoreHistoryEntry[]} [score_history] - History of score adjustments with timestamps
 * @property {'standard'|'practice'} [match_type] - "standard" (two players) or "practice" (single player, racks count)
 */

/**
 * List all pool matches. Uses fetchWithAuth when available (token sent if logged in).
 * @returns {Promise<PoolMatch[]>}
 */
export async function listMatches() {
  const res = await fetchWithAuth('/api/pool-matches')
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to list matches')
  }
  return res.json()
}

/**
 * @param {string} matchId
 * @returns {Promise<PoolMatch>}
 */
export async function getMatch(matchId) {
  const res = await fetchWithAuth(`/api/pool-matches/${matchId}`)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch match')
  }
  return res.json()
}

/**
 * @param {string} cameraId
 * @returns {Promise<PoolMatch | null>}
 */
export async function getActiveMatch(cameraId) {
  const res = await fetchWithAuth(`/api/pool-matches/active?camera_id=${encodeURIComponent(cameraId)}`)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch active match')
  }
  const data = await res.json()
  return data
}

/**
 * @param {{ match_type?: 'standard'|'practice', player_one: { name: string, race_to: number, rating?: { type: string, value: number } }, player_two?: { name: string, race_to: number, rating?: { type: string, value: number } }, camera_id: string, description?: string }} payload
 * @returns {Promise<{ id: string }>}
 */
export async function createMatch(payload) {
  const res = await fetchWithAuth('/api/pool-matches', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to create match')
  }
  return res.json()
}

/**
 * @param {string} matchId
 * @param {1 | 2} player
 * @param {number} gamesWon
 * @returns {Promise<PoolMatch>}
 */
export async function updateScore(matchId, player, gamesWon) {
  const res = await fetchWithAuth(`/api/pool-matches/${matchId}/score`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ player, games_won: gamesWon }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to update score')
  }
  return res.json()
}

/**
 * @param {string} matchId
 * @returns {Promise<void>}
 */
export async function deleteMatch(matchId) {
  const res = await fetchWithAuth(`/api/pool-matches/${matchId}`, { method: 'DELETE' })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to delete match')
  }
}

/**
 * Download recording for a game (score history entry).
 * @param {string} cameraId
 * @param {number} startMs - Start time in milliseconds (match start or prev score timestamp)
 * @param {number} durationSec - Duration in seconds
 * @param {string} filename - Suggested filename for download
 * @returns {Promise<void>}
 */
export async function downloadGameRecording(cameraId, startMs, durationSec, filename = 'game.mp4') {
  const url = `/api/cameras/${encodeURIComponent(cameraId)}/recordings/download?start=${startMs}&duration=${durationSec}`
  const res = await fetchWithAuth(url)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to download recording')
  }
  const blob = await res.blob()
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = filename
  a.click()
  URL.revokeObjectURL(a.href)
}

/**
 * @param {string} matchId
 * @returns {Promise<PoolMatch>}
 */
export async function endMatch(matchId) {
  const res = await fetchWithAuth(`/api/pool-matches/${matchId}/end`, {
    method: 'PATCH',
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to end match')
  }
  return res.json()
}
