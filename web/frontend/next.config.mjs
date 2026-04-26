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
    return [
      {
        source: '/api/:path*',
        destination: `${apiUrl}/api/:path*`,
      },
    ]
  },

  // Disable response buffering for SSE to work properly
  experimental: {
    proxyTimeout: 300_000, // 5 minutes for long-lived SSE connections
  },
}

export default nextConfig
