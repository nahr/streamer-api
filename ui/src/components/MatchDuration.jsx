import { useState, useEffect } from 'react'
import { formatDuration } from '../utils/format.js'

/**
 * Displays match duration, updating every second for ongoing matches.
 * @param {{ match: { start_time: number, end_time?: number } }} props
 */
export function MatchDuration({ match }) {
  const [now, setNow] = useState(Date.now())
  useEffect(() => {
    if (match.end_time) return
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [match.end_time])
  const endMs = match.end_time ?? now
  const durationMs = endMs - match.start_time
  return <>{formatDuration(durationMs)}</>
}
