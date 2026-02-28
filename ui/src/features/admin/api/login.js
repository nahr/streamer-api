/**
 * Authenticates an admin by email and password.
 * @param {string} email
 * @param {string} password
 * @returns {Promise<{ ok: boolean }>}
 * @throws {Error} When login fails (invalid credentials)
 */
export async function loginAdmin(email, password) {
  const res = await fetch('/api/admin/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Login failed')
  }
  return res.json()
}
