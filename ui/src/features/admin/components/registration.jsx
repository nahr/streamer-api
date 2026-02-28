import { useState } from 'react'
import { createAdmin } from '../api/create'
import './admin.css'

/**
 * Registration form for creating the first admin account.
 * @param {Object} props
 * @param {() => void} props.onSuccess - Called after successful registration
 */
export function Registration({ onSuccess }) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e) => {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      await createAdmin(email, password)
      onSuccess?.()
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="admin-form-container">
      <h1>Create Admin Account</h1>
      <p>No admin exists yet. Create the first admin to get started.</p>
      <form onSubmit={handleSubmit} className="admin-form">
        <input
          type="email"
          placeholder="Email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
          autoComplete="email"
        />
        <input
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
          autoComplete="new-password"
        />
        {error && <p className="admin-form-error">{error}</p>}
        <button type="submit" disabled={loading}>
          {loading ? 'Creating...' : 'Sign up'}
        </button>
      </form>
    </div>
  )
}
