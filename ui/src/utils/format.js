/**
 * Shared formatting utilities for dates, durations, and match data.
 */

/**
 * Format a timestamp (ms since epoch) for display.
 * @param {number} ms
 * @param {'short'|'medium'|'full'|'withSeconds'} [style='medium'] - short: time only; medium: date+time; full: month/day/hour/min; withSeconds: date+time with seconds
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
  if (style === 'withSeconds') {
    return d.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
      second: '2-digit',
    })
  }
  return d.toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  })
}

/**
 * Format timestamp for use in filenames: YYYY-MM-DD_HH-mm-ss
 * @param {number} ms
 * @returns {string}
 */
export function formatTimestampForFilename(ms) {
  const d = new Date(ms)
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const h = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  const sec = String(d.getSeconds()).padStart(2, '0')
  return `${y}-${m}-${day}_${h}-${min}-${sec}`
}

/**
 * Format recording download filename: {start-datetime}-(practice|match)-{number}.mp4
 * @param {number} startMs
 * @param {'practice'|'standard'} matchType
 * @param {number} number - rack number for practice, game number for match
 * @returns {string}
 */
export function formatRecordingFilename(startMs, matchType, number) {
  const prefix = formatTimestampForFilename(startMs)
  const type = matchType === 'practice' ? 'practice' : 'match'
  return `${prefix}-${type}-${number}.mp4`
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
 * Parse record_delete_after (e.g. "24h", "7d") to milliseconds. Returns Infinity if keep forever.
 * @param {string} recordDeleteAfter
 * @returns {number}
 */
function parseRetentionMs(recordDeleteAfter) {
  const s = (recordDeleteAfter || '').trim()
  if (!s || s === '0') return Infinity
  const m = s.match(/^(\d+)([hd])$/i)
  if (!m) return 24 * 60 * 60 * 1000 // default 24h
  const n = parseInt(m[1], 10)
  const unit = m[2].toLowerCase()
  if (unit === 'h') return n * 60 * 60 * 1000
  if (unit === 'd') return n * 24 * 60 * 60 * 1000
  return 24 * 60 * 60 * 1000
}

/**
 * Check if a recording ending at endTimestampMs is still within retention.
 * @param {number} endTimestampMs - End time of the recording (ms since epoch)
 * @param {string} recordDeleteAfter - e.g. "24h", "7d", "" for keep forever
 * @returns {boolean}
 */
export function isRecordingAvailable(endTimestampMs, recordDeleteAfter) {
  const retentionMs = parseRetentionMs(recordDeleteAfter)
  if (retentionMs === Infinity) return true
  return Date.now() - endTimestampMs < retentionMs
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
