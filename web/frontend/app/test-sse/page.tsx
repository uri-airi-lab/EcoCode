'use client'

import { useEffect, useState } from 'react'

export default function TestSSEPage() {
  const [messages, setMessages] = useState<string[]>([])
  const [status, setStatus] = useState<string>('Not connected')
  const [url, setUrl] = useState<string>('/api/sse')

  useEffect(() => {
    console.log('[Test SSE] Attempting to connect to:', url)
    setStatus('Connecting...')
    
    const eventSource = new EventSource(url)

    eventSource.onopen = () => {
      console.log('[Test SSE] Connection opened!')
      setStatus('Connected ✅')
    }

    eventSource.onmessage = (event) => {
      console.log('[Test SSE] Message received:', event.data)
      setMessages(prev => [...prev, `${new Date().toISOString()}: ${event.data}`])
    }

    eventSource.onerror = (err) => {
      console.error('[Test SSE] Error:', err)
      console.log('[Test SSE] ReadyState:', eventSource.readyState)
      
      if (eventSource.readyState === EventSource.CONNECTING) {
        setStatus('Reconnecting...')
      } else if (eventSource.readyState === EventSource.CLOSED) {
        setStatus('Disconnected ❌')
      }
    }

    return () => {
      console.log('[Test SSE] Cleaning up connection')
      eventSource.close()
    }
  }, [url])

  return (
    <div className="p-8">
      <h1 className="text-2xl font-bold mb-4">SSE Connection Test</h1>
      
      <div className="mb-4">
        <label className="block mb-2">SSE URL:</label>
        <input
          type="text"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          className="border p-2 w-full max-w-md"
        />
      </div>

      <div className="mb-4">
        <strong>Status:</strong> <span className="ml-2">{status}</span>
      </div>

      <div className="mb-4">
        <strong>Messages received:</strong> {messages.length}
      </div>

      <div className="border p-4 bg-gray-50 max-h-96 overflow-y-auto">
        {messages.length === 0 ? (
          <p className="text-gray-500">No messages yet...</p>
        ) : (
          <ul className="space-y-2">
            {messages.map((msg, i) => (
              <li key={i} className="text-sm font-mono break-all">
                {msg}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="mt-4 text-sm text-gray-600">
        <p>Open the browser console to see detailed logs.</p>
        <p>Try these URLs:</p>
        <ul className="list-disc ml-6 mt-2">
          <li><code>/api/sse</code> - Through Next.js proxy</li>
          <li><code>http://localhost:3001/api/sse</code> - Direct to backend</li>
        </ul>
      </div>
    </div>
  )
}
