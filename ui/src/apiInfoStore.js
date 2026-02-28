import { useState, useEffect } from 'react'

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
 * @returns {{ initialized: boolean | null, loading: boolean, error: Error | null, refetch: () => Promise<void> }}
 */
export function useApiInfo() {
  const [initialized, setInitialized] = useState(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)

  const refetch = async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await fetchApiInfo()
      setInitialized(data.initialized)
    } catch (err) {
      setError(err)
      setInitialized(false) // show signup on error so user can retry
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    refetch()
  }, [])

  return { initialized, loading, error, refetch }
}
