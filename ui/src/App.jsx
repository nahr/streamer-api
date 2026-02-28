import { useState, useEffect } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'

import { useApiInfo } from './apiInfoStore'
import { Registration } from './features/admin'

function App() {
  const [count, setCount] = useState(0)
  const [message, setMessage] = useState('')
  const { initialized, loading, error, refetch } = useApiInfo()

  useEffect(() => {
    if (initialized !== true) return
    fetch('/api/hello')
      .then((res) => res.text())
      .then(setMessage)
      .catch(() => setMessage('Failed to fetch'))
  }, [initialized])

  if (loading) {
    return <p className="api-message">Loading...</p>
  }

  if (error) {
    return (
      <p className="api-message">
        Failed to connect. Is the API running?{' '}
        <button type="button" onClick={refetch}>
          Retry
        </button>
      </p>
    )
  }

  if (!initialized) {
    return <Registration onSuccess={refetch} />
  }

  return (
    <>
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
