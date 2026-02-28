import { hashPassword } from './hashPassword'

/**
 * Creates the first admin account.
 * Password is hashed client-side before sending.
 * @param {string} email
 * @param {string} password - Plaintext password (hashed before send)
 * @returns {Promise<void>}
 * @throws {Error} When signup fails (e.g. admin already exists)
 */
export async function createAdmin(email, password) {
  const passwordHash = await hashPassword(password)
  const res = await fetch('/api/admin', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password: passwordHash }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Signup failed')
  }
}
