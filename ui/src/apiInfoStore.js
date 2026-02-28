import { useState, useEffect, useRef } from 'react'

const RETRY_INTERVAL_MS = 5000

/**
 * Fetches API info from /api/info.
 * @returns {Promise<{ initialized: boolean }>}
 */
export async function fetchApiInfo() {
  const res = await fetch('/api/info')
  if (!res.ok) throw new Error('Failed to fetch API info')
  return res.json()
}

/**
 * Hook that fetches API info on mount.
 * On failure, shows a spinner and retries every 5 seconds until success.
 * @returns {{ initialized: boolean | null, loading: boolean, retrying: boolean, refetch: () => Promise<void> }}
 */
export function useApiInfo() {
  const [initialized, setInitialized] = useState(null)
  const [loading, setLoading] = useState(true)
  const [retrying, setRetrying] = useState(false)
  const intervalRef = useRef(null)

  const attemptFetch = async () => {
    try {
      const data = await fetchApiInfo()
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
      setInitialized(data.initialized)
      setRetrying(false)
      setLoading(false)
    } catch {
      setRetrying(true)
      setLoading(true) // keep spinner visible while retrying
    }
  }

  const refetch = async () => {
    setLoading(true)
    setRetrying(false)
    if (intervalRef.current) {
      clearInterval(intervalRef.current)
      intervalRef.current = null
    }
    await attemptFetch()
  }

  useEffect(() => {
    attemptFetch()
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
      }
    }
  }, [])

  useEffect(() => {
    if (retrying && !intervalRef.current) {
      intervalRef.current = setInterval(attemptFetch, RETRY_INTERVAL_MS)
    }
  }, [retrying])

  return { initialized, loading, retrying, refetch }
}
