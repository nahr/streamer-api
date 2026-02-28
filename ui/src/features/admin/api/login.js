import { hashPassword } from './hashPassword'

/**
 * Authenticates an admin by email and password.
 * Password is hashed client-side before sending.
 * @param {string} email
 * @param {string} password - Plaintext password (hashed before send)
 * @returns {Promise<{ ok: boolean }>}
 * @throws {Error} When login fails (invalid credentials)
 */
export async function loginAdmin(email, password) {
  const passwordHash = await hashPassword(password)
  const res = await fetch('/api/admin/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password: passwordHash }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Login failed')
  }
  return res.json()
}
