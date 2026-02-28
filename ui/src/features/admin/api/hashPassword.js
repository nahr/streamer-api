/**
 * Hashes a password with SHA-256 using the Web Crypto API.
 * The plaintext password never leaves the client.
 * @param {string} password - Plaintext password
 * @returns {Promise<string>} Hex-encoded SHA-256 hash
 */
export async function hashPassword(password) {
  const encoder = new TextEncoder()
  const data = encoder.encode(password)
  const hashBuffer = await crypto.subtle.digest('SHA-256', data)
  const hashArray = Array.from(new Uint8Array(hashBuffer))
  return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('')
}
