export function detectBasePath() {
  const pathname = window.location.pathname || '/'
  const appMatch = pathname.match(/^(.*?\/)app(?:\/.*)?$/)
  if (appMatch?.[1]) {
    return appMatch[1]
  }

  if (pathname === '/') {
    return '/'
  }

  if (pathname.endsWith('/')) {
    return pathname
  }

  const segments = pathname.split('/').filter(Boolean)
  if (segments.length <= 1) {
    return `${pathname}/`
  }

  return pathname.slice(0, pathname.lastIndexOf('/') + 1)
}

export function withBasePath(path) {
  const base = detectBasePath()
  const normalizedPath = path.startsWith('/') ? path.slice(1) : path
  return normalizedPath ? `${base}${normalizedPath}` : base
}

export function currentRelativePath() {
  const base = detectBasePath()
  const pathname = window.location.pathname || '/'
  if (!pathname.startsWith(base)) {
    return pathname
  }

  const relativePath = pathname.slice(Math.max(0, base.length - 1))
  return relativePath || '/'
}
