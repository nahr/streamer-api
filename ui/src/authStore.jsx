import { createContext, useContext, useState, useEffect, useCallback } from 'react'
import { useAuth0 } from '@auth0/auth0-react'
import { setTokenGetter } from './apiClient.js'

/**
 * Fetches current user from backend (validates token, syncs to DB, returns isAdmin).
 * Sends both ID token (for validation) and access token (for userinfo when JWT lacks profile).
 * Auth0's userinfo endpoint requires an access token; ID tokens are rejected.
 * @param {string} idToken - Bearer token (ID token for validation)
 * @param {string} [accessToken] - Optional access token for userinfo (needed for Facebook/social when skipAudience)
 * @returns {Promise<{ sub: string, email: string, name: string, picture?: string, is_admin: boolean }>}
 */
export async function fetchAuthMe(idToken, accessToken) {
  const headers = {
    Authorization: `Bearer ${idToken}`,
    ...(accessToken && accessToken !== idToken && { 'X-Auth0-Access-Token': accessToken }),
  }
  const res = await fetch('/api/auth/me', { headers })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Failed to fetch user')
  }
  return res.json()
}

const AuthContext = createContext(null)

/**
 * Provider that wraps Auth0 and fetches our user info (including isAdmin) from the backend.
 * Must be used inside Auth0Provider.
 * @param {{ children: React.ReactNode, skipAudience?: boolean }} props
 */
export function AuthProvider({ children, skipAudience = false }) {
  const useIdToken = skipAudience
  const { isAuthenticated, getAccessTokenSilently, getIdTokenClaims, logout: auth0Logout } = useAuth0()
  const [user, setUser] = useState(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)

  const loadUser = useCallback(async () => {
    if (!isAuthenticated) {
      setUser(null)
      setLoading(false)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const idToken = (await getIdTokenClaims())?.__raw
      // Use cache only – avoid forcing refresh; user re-authenticates only when cached token expires
      const accessToken = await getAccessTokenSilently({ cacheMode: 'cache' }).catch(() => null)
      const token = useIdToken ? idToken : accessToken
      if (!token) throw new Error('No token available')
      // When skipAudience, send access token for userinfo (Auth0 rejects ID tokens)
      const data = await fetchAuthMe(token, useIdToken ? accessToken : null)
      setUser(data)
    } catch (err) {
      setError(err.message)
      setUser(null)
    } finally {
      setLoading(false)
    }
  }, [isAuthenticated, getAccessTokenSilently, getIdTokenClaims, useIdToken])

  useEffect(() => {
    loadUser()
  }, [loadUser])

  // Register token getter for apiClient so all API calls include the Bearer token
  useEffect(() => {
    if (!isAuthenticated) {
      setTokenGetter(null)
      return
    }
    setTokenGetter(async () => {
      try {
        if (useIdToken) {
          const claims = await getIdTokenClaims()
          return claims?.__raw ?? null
        }
        return await getAccessTokenSilently()
      } catch {
        return null
      }
    })
    return () => setTokenGetter(null)
  }, [isAuthenticated, getAccessTokenSilently, getIdTokenClaims, useIdToken])

  const logout = useCallback(() => {
    setUser(null)
    auth0Logout({ logoutParams: { returnTo: window.location.origin } })
  }, [auth0Logout])

  const refetch = useCallback(() => {
    loadUser()
  }, [loadUser])

  const value = {
    user,
    loading,
    error,
    isLoggedIn: !!user,
    isAdmin: user?.is_admin ?? false,
    logout,
    refetch,
  }

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

/**
 * Hook that provides auth state. Must be used within AuthProvider (inside Auth0Provider).
 * @returns {{ user: { sub, email, name, picture?, is_admin } | null, loading: boolean, error: string | null, isLoggedIn: boolean, isAdmin: boolean, logout: () => void, refetch: () => void }}
 */
export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}
