import { NextResponse } from 'next/server'
import type { NextRequest } from 'next/server'

export function middleware(request: NextRequest) {
  const token = request.cookies.get('auth_token')
  const { pathname } = request.nextUrl
  
  const isAuthPage = pathname === '/login' || pathname === '/register'
  const isApiRoute = pathname.startsWith('/api')
  const isStaticRoute = pathname.startsWith('/_next') || pathname === '/favicon.ico'

  if (isApiRoute || isStaticRoute) {
    return NextResponse.next()
  }

  if (!token && !isAuthPage) {
    return NextResponse.redirect(new URL('/login', request.url))
  }

  if (token && isAuthPage) {
    // We don't know the workspace slug here, so we redirect to / 
    // which should ideally fetch user workspaces and redirect to the first one
    return NextResponse.redirect(new URL('/', request.url))
  }

  return NextResponse.next()
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
}
