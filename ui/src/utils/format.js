/**
 * Shared formatting utilities for dates, durations, and match data.
 */

/**
 * Format a timestamp (ms since epoch) for display.
 * @param {number} ms
 * @param {'short'|'medium'|'full'} [style='medium'] - short: time only; medium: date+time; full: month/day/hour/min
 * @returns {string}
 */
export function formatTime(ms, style = 'medium') {
  const d = new Date(ms)
  if (style === 'short') {
    return d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
  }
  if (style === 'full') {
    return d.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    })
  }
  return d.toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  })
}

/**
 * Format a duration in milliseconds.
 * @param {number} ms
 * @param {{ includeSeconds?: boolean }} [options] - includeSeconds: when false, omit seconds when hours > 0 (default: true)
 * @returns {string}
 */
export function formatDuration(ms, options = {}) {
  const { includeSeconds = true } = options
  const totalSeconds = Math.floor(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) {
    return includeSeconds
      ? `${hours}h ${minutes}m ${seconds}s`
      : `${hours}h ${minutes}m`
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

/**
 * Get the winner of a completed match, or null if none/ongoing.
 * Practice matches have no winner.
 * @param {{ end_time?: number, match_type?: string, player_one: { games_won: number, race_to: number, name: string }, player_two: { games_won: number, race_to: number, name: string } }} match
 * @returns {string|null}
 */
export function getMatchWinner(match) {
  if (!match.end_time) return null
  if (match?.match_type === 'practice') return null
  if (match.player_one.games_won >= match.player_one.race_to) return match.player_one.name
  if (match.player_two.games_won >= match.player_two.race_to) return match.player_two.name
  return null
}

/**
 * Format match winner for display: "Name won" or "Ended early".
 * @param {Parameters<typeof getMatchWinner>[0]} match
 * @returns {string}
 */
export function formatMatchWinner(match) {
  const winner = getMatchWinner(match)
  return winner ? `${winner} won` : 'Ended early'
}

/**
 * Format match title for display: "X vs Y" or "Practice: X | N racks".
 * @param {{ match_type?: string, player_one: { name: string, games_won: number }, player_two: { name: string } }} match
 * @returns {string}
 */
export function formatMatchTitle(match) {
  if (match?.match_type === 'practice') {
    const racks = match.player_one.games_won
    return `Practice: ${match.player_one.name}${racks > 0 ? ` | ${racks} rack${racks !== 1 ? 's' : ''}` : ''}`
  }
  return `${match.player_one.name} vs ${match.player_two.name}`
}
