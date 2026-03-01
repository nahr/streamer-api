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
 */

/**
 * List all pool matches. Public endpoint (no auth required).
 * @returns {Promise<PoolMatch[]>}
 */
export async function listMatches() {
  const res = await fetch('/api/pool-matches')
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to list matches')
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
 * @param {{ player_one: { name: string, race_to: number, rating?: { type: string, value: number } }, player_two: { name: string, race_to: number, rating?: { type: string, value: number } }, camera_id: string, description?: string }} payload
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
