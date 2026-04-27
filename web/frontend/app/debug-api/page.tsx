'use client'

import { useState } from 'react'

export default function DebugApiPage() {
  const [results, setResults] = useState<Record<string, any>>({})
  const [loading, setLoading] = useState<Record<string, boolean>>({})

  const testEndpoint = async (name: string, url: string) => {
    setLoading(prev => ({ ...prev, [name]: true }))
    const startTime = Date.now()
    
    try {
      const response = await fetch(url, { credentials: 'include' })
      const endTime = Date.now()
      const duration = endTime - startTime
      
      if (response.ok) {
        const data = await response.json()
        setResults(prev => ({
          ...prev,
          [name]: {
            status: 'success',
            duration: `${duration}ms`,
            dataSize: JSON.stringify(data).length,
            recordCount: Array.isArray(data) ? data.length : 'N/A'
          }
        }))
      } else {
        setResults(prev => ({
          ...prev,
          [name]: {
            status: 'error',
            duration: `${duration}ms`,
            error: `${response.status} ${response.statusText}`
          }
        }))
      }
    } catch (error) {
      const endTime = Date.now()
      const duration = endTime - startTime
      setResults(prev => ({
        ...prev,
        [name]: {
          status: 'error',
          duration: `${duration}ms`,
          error: error instanceof Error ? error.message : 'Unknown error'
        }
      }))
    } finally {
      setLoading(prev => ({ ...prev, [name]: false }))
    }
  }

  const tests = [
    { name: 'Backend Health', url: '/api/hello' },
    { name: 'Projects', url: '/api/projects' },
    { name: 'All Runs', url: '/api/runs' },
    { name: 'SSE Connection', url: '/api/sse' },
  ]

  return (
    <div className="p-8 max-w-4xl mx-auto">
      <h1 className="text-2xl font-bold mb-6">API Debug Dashboard</h1>
      
      <div className="grid gap-4 mb-8">
        {tests.map(test => (
          <div key={test.name} className="border rounded-lg p-4">
            <div className="flex items-center justify-between mb-2">
              <h3 className="font-semibold">{test.name}</h3>
              <button
                onClick={() => testEndpoint(test.name, test.url)}
                disabled={loading[test.name]}
                className="px-4 py-2 bg-blue-500 text-white rounded disabled:opacity-50"
              >
                {loading[test.name] ? 'Testing...' : 'Test'}
              </button>
            </div>
            
            <p className="text-sm text-gray-600 mb-2">{test.url}</p>
            
            {results[test.name] && (
              <div className={`p-3 rounded text-sm ${
                results[test.name].status === 'success' 
                  ? 'bg-green-100 text-green-800' 
                  : 'bg-red-100 text-red-800'
              }`}>
                <div><strong>Status:</strong> {results[test.name].status}</div>
                <div><strong>Duration:</strong> {results[test.name].duration}</div>
                {results[test.name].dataSize && (
                  <div><strong>Data Size:</strong> {results[test.name].dataSize} bytes</div>
                )}
                {results[test.name].recordCount && (
                  <div><strong>Records:</strong> {results[test.name].recordCount}</div>
                )}
                {results[test.name].error && (
                  <div><strong>Error:</strong> {results[test.name].error}</div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-2">Environment Info</h3>
        <div className="text-sm space-y-1">
          <div><strong>API URL:</strong> {process.env.NEXT_PUBLIC_API_URL || 'Not set'}</div>
          <div><strong>User Agent:</strong> {typeof window !== 'undefined' ? navigator.userAgent : 'Server-side'}</div>
          <div><strong>Current Time:</strong> {new Date().toISOString()}</div>
        </div>
      </div>
    </div>
  )
}