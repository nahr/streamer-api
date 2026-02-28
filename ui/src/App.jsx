import { useState, useEffect } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'

import { useApiInfo } from './apiInfoStore'
import { useAuth } from './authStore'
import { Registration, Login } from './features/admin'

function App() {
  const [count, setCount] = useState(0)
  const [message, setMessage] = useState('')
  const { initialized, loading, retrying, refetch } = useApiInfo()
  const { isLoggedIn, login, logout } = useAuth()

  useEffect(() => {
    if (initialized !== true) return
    fetch('/api/hello')
      .then((res) => res.text())
      .then(setMessage)
      .catch(() => setMessage('Failed to fetch'))
  }, [initialized])

  if (loading) {
    return (
      <div className="api-loading">
        <div className="spinner" aria-hidden />
        <p className="api-message">{retrying ? 'Connecting... Retrying every 5 seconds.' : 'Loading...'}</p>
      </div>
    )
  }

  if (!initialized) {
    return <Registration onSuccess={refetch} />
  }

  if (!isLoggedIn) {
    return <Login onSuccess={login} />
  }

  return (
    <>
      <div style={{ position: 'absolute', top: 16, right: 16 }}>
        <button type="button" onClick={logout} style={{ fontSize: '0.9rem', padding: '0.4rem 0.8rem' }}>
          Log out
        </button>
      </div>
      <div>
        <a href="https://vite.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React</h1>
      <p className="api-message">{message || 'Loading...'}</p>
      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
        <p>
          Edit <code>src/App.jsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </>
  )
}

export default App
