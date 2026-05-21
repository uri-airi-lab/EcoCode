/** @type {import('next').NextConfig} */
const nextConfig = {
  typescript: {
    ignoreBuildErrors: true,
  },

  // Required for the multi-stage Docker build (copies only the standalone bundle)
  output: 'standalone',

  async rewrites() {
    // In Docker the backend is reachable via the service name "backend".
    // Locally it stays on http://localhost:3001.
    const apiUrl = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'
    return {
      // beforeFiles: run before filesystem routes — catch-all API proxy
      beforeFiles: [],
      // afterFiles: run after filesystem routes — /api/sse is handled by its
      // own route handler (app/api/sse/route.ts) and must NOT be rewritten here.
      afterFiles: [
        {
          source: '/api/:path*',
          destination: `${apiUrl}/api/:path*`,
        },
      ],
      fallback: [],
    }
  },

  async headers() {
    return [
      {
        // HTML pages must never be served from cache so chunk URLs are always fresh
        source: '/((?!_next/static|_next/image|favicon.ico).*)',
        headers: [
          { key: 'Cache-Control', value: 'no-cache, no-store, must-revalidate' },
          { key: 'X-Frame-Options', value: 'DENY' },
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
          { key: 'Content-Security-Policy', value: "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ws: wss: http: https:;" },
        ],
      },
    ]
  },

  // Disable response buffering for SSE to work properly
  experimental: {
    proxyTimeout: 300_000, // 5 minutes for long-lived SSE connections
  },
}

export default nextConfig
