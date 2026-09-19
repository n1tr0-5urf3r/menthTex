import { useEffect, useRef, useState } from 'react'

export default function LoginProviderPopup({
  providers,
  onSelectProvider,
  buttonClassName = 'btn btn-outline-secondary btn-sm',
}) {
  const [open, setOpen] = useState(false)
  const rootRef = useRef(null)
  const configuredProviders = providers?.length
    ? providers
    : [{ id: 'gitlab', label: 'GitLab' }]

  useEffect(() => {
    if (!open) {
      return undefined
    }

    function handlePointerDown(event) {
      if (!rootRef.current?.contains(event.target)) {
        setOpen(false)
      }
    }

    function handleKeyDown(event) {
      if (event.key === 'Escape') {
        setOpen(false)
      }
    }

    document.addEventListener('pointerdown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  return (
    <div className="login-provider-root" ref={rootRef}>
      <button
        type="button"
        className={buttonClassName}
        onClick={() => setOpen((current) => !current)}
        aria-haspopup="menu"
        aria-expanded={open}
      >
        Sign in
      </button>
      {open ? (
        <div className="login-provider-popover" role="menu">
          {configuredProviders.map((provider) => (
            <button
              type="button"
              className="btn btn-outline-secondary btn-sm login-provider-option"
              key={provider.id}
              onClick={() => {
                setOpen(false)
                onSelectProvider(provider.id)
              }}
              role="menuitem"
            >
              Sign in with {provider.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}
