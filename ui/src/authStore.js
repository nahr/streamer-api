import { useState, useEffect } from 'react'

const AUTH_KEY = 'admin_logged_in'

function getStoredAuth() {
  try {
    return localStorage.getItem(AUTH_KEY) === 'true'
  } catch {
    return false
  }
}

function setStoredAuth(loggedIn) {
  try {
    if (loggedIn) {
      localStorage.setItem(AUTH_KEY, 'true')
    } else {
      localStorage.removeItem(AUTH_KEY)
    }
  } catch {
    // ignore
  }
}

/**
 * Hook that tracks admin login state.
 * @returns {{ isLoggedIn: boolean, login: () => void, logout: () => void }}
 */
export function useAuth() {
  const [isLoggedIn, setIsLoggedIn] = useState(getStoredAuth)

  const login = () => {
    setStoredAuth(true)
    setIsLoggedIn(true)
  }

  const logout = () => {
    setStoredAuth(false)
    setIsLoggedIn(false)
  }

  // Sync with other tabs
  useEffect(() => {
    const handler = (e) => {
      if (e.key === AUTH_KEY) {
        setIsLoggedIn(getStoredAuth())
      }
    }
    window.addEventListener('storage', handler)
    return () => window.removeEventListener('storage', handler)
  }, [])

  return { isLoggedIn, login, logout }
}
