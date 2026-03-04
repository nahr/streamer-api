import { createContext, useContext, useState, useEffect, useRef } from 'react'

const RETRY_INTERVAL_MS = 5000

/**
 * Fetches API info from /api/info.
 * @returns {Promise<{ initialized: boolean, location_name?: string, has_users?: boolean, cameras_configured?: boolean }>}
 */
export async function fetchApiInfo() {
  const res = await fetch('/api/info')
  if (!res.ok) throw new Error('Failed to fetch API info')
  return res.json()
}

const ApiInfoContext = createContext(null)

/**
 * Provider for API info. Wrap the app so useApiInfo shares state.
 */
export function ApiInfoProvider({ children }) {
  const [initialized, setInitialized] = useState(null)
  const [locationName, setLocationName] = useState('')
  const [hasUsers, setHasUsers] = useState(false)
  const [camerasConfigured, setCamerasConfigured] = useState(false)
  const [recordDeleteAfter, setRecordDeleteAfter] = useState('')
  const [version, setVersion] = useState('')
  const [upToDate, setUpToDate] = useState(true)
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
      setLocationName(data.location_name || '')
      setHasUsers(data.has_users ?? false)
      setCamerasConfigured(data.cameras_configured ?? false)
      setRecordDeleteAfter(data.record_delete_after || '')
      setVersion(data.version || '')
      setUpToDate(data.up_to_date ?? true)
      setRetrying(false)
      setLoading(false)
    } catch {
      setRetrying(true)
      setLoading(true) // keep spinner visible while retrying
    }
  }

  const refetch = async (options = {}) => {
    const { silent = false } = options
    if (!silent) setLoading(true)
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

  return (
    <ApiInfoContext.Provider value={{ initialized, locationName, hasUsers, camerasConfigured, recordDeleteAfter, version, upToDate, loading, retrying, refetch }}>
      {children}
    </ApiInfoContext.Provider>
  )
}

/**
 * Hook that provides API info from context.
 * @returns {{ initialized: boolean | null, locationName: string, hasUsers: boolean, camerasConfigured: boolean, recordDeleteAfter: string, loading: boolean, retrying: boolean, refetch: () => Promise<void> }}
 */
export function useApiInfo() {
  const ctx = useContext(ApiInfoContext)
  if (!ctx) throw new Error('useApiInfo must be used within ApiInfoProvider')
  return ctx
}
