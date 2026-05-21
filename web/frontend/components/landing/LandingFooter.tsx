import Link from 'next/link'
import { Github, ExternalLink } from 'lucide-react'

const FOOTER_LINKS = [
  {
    title: 'Product',
    links: [
      { label: 'Features',     href: '#features' },
      { label: 'Comparison',   href: '#comparison' },
      { label: 'Architecture', href: '#architecture' },
      { label: 'Integrations', href: '#integrations' },
    ],
  },
  {
    title: 'Resources',
    links: [
      { label: 'Documentation',   href: '#' },
      { label: 'Python SDK',      href: '#integrations' },
      { label: 'CLI Reference',   href: '#usage' },
      { label: 'Roadmap',         href: '#roadmap' },
    ],
  },
  {
    title: 'Platform',
    links: [
      { label: 'Dashboard',  href: '/login', external: false },
      { label: 'Sign In',    href: '/login' },
      { label: 'Register',   href: '/register' },
      { label: 'GitHub',     href: '#', external: true },
    ],
  },
]

export function LandingFooter() {
  return (
    <footer className="relative border-t border-border bg-card/30 backdrop-blur-sm">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-14">
        {/* Top row */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-10 mb-12">
          {/* Brand */}
          <div className="col-span-2 sm:col-span-1 flex flex-col gap-4">
            <div className="flex items-center gap-2.5">
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img
                src="/ecocode-logo.png"
                alt="EcoCode logo"
                width={28}
                height={28}
                className="rounded-md"
              />
              <span className="font-bold text-foreground">EcoCode</span>
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed max-w-xs">
              High-precision, multi-GPU energy monitoring for sustainable software development.
            </p>
            <a
              href="#"
              aria-label="EcoCode on GitHub"
              className="inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              <Github className="w-4 h-4" />
              View on GitHub
            </a>
          </div>

          {/* Link columns */}
          {FOOTER_LINKS.map(({ title, links }) => (
            <div key={title}>
              <p className="text-xs font-semibold text-foreground tracking-widest uppercase mb-4">{title}</p>
              <ul className="space-y-2.5">
                {links.map(({ label, href, external }) => (
                  <li key={label}>
                    {external ? (
                      <a
                        href={href}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {label}
                        <ExternalLink className="w-3 h-3 opacity-50" />
                      </a>
                    ) : (
                      <Link
                        href={href}
                        className="text-sm text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {label}
                      </Link>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {/* Bottom bar */}
        <div className="pt-6 border-t border-border/50 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-muted-foreground">
          <p>© {new Date().getFullYear()} EcoCode. All rights reserved.</p>
          <div className="flex items-center gap-4">
            <span>Linux only · CPU · GPU · Memory</span>
          </div>
        </div>
      </div>
    </footer>
  )
}
