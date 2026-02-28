/**
 * Creates the first admin account.
 * @param {string} email
 * @param {string} password
 * @returns {Promise<void>}
 * @throws {Error} When signup fails (e.g. admin already exists)
 */
export async function createAdmin(email, password) {
  const res = await fetch('/api/admin', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || 'Signup failed')
  }
}
