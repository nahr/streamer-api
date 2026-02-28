import { useState, useEffect, useCallback } from 'react'
import { listCameras } from './api/cameras.js'

/**
 * Hook that fetches the camera list on mount.
 * @returns {{ cameras: Array<{ id: string, name: string, camera_type: object }>, loading: boolean, error: string, refetch: () => Promise<void> }}
 */
export function useCameras() {
  const [cameras, setCameras] = useState([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')

  const refetch = useCallback(async () => {
    setLoading(true)
    setError('')
    try {
      const data = await listCameras()
      setCameras(data)
    } catch (err) {
      setError(err.message)
      setCameras([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    refetch()
  }, [refetch])

  return { cameras, loading, error, refetch }
}
