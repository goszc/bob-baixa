export function IconPlay({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 20 20" className={className} aria-hidden="true">
      <path d="M7.2 4.4v11.2L16.2 10 7.2 4.4Z" fill="currentColor" />
    </svg>
  )
}

export function IconPause({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 20 20" className={className} aria-hidden="true">
      <path d="M6.2 4.2h2.4v11.6H6.2V4.2Zm5.2 0h2.4v11.6h-2.4V4.2Z" fill="currentColor" />
    </svg>
  )
}

export function IconPrev({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 20 20" className={className} aria-hidden="true">
      <path d="M5.2 4.4h1.8v11.2H5.2V4.4Zm10.2 0L8.6 10l6.8 5.6V4.4Z" fill="currentColor" />
    </svg>
  )
}

export function IconNext({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 20 20" className={className} aria-hidden="true">
      <path d="M4.6 4.4 11.4 10 4.6 15.6V4.4Zm8.4 0h1.8v11.2h-1.8V4.4Z" fill="currentColor" />
    </svg>
  )
}

export function IconShuffle({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" aria-hidden="true">
      <path d="M16 3h5v5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M4 20 21 3" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      <path d="M21 16v5h-5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      <path d="m15 15 6 6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      <path d="m4 4 5 5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  )
}

export function IconRepeat({ className, one }: { className?: string; one?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" aria-hidden="true">
      <path d="m17 1 4 4-4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M3 11V9a4 4 0 0 1 4-4h14" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      <path d="m7 23-4-4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M21 13v2a4 4 0 0 1-4 4H3" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      {one ? (
        <text x="12" y="14.2" textAnchor="middle" fontSize="8" fontWeight="700" fill="currentColor" stroke="none">
          1
        </text>
      ) : null}
    </svg>
  )
}

export function IconVolume({ className, muted }: { className?: string; muted?: boolean }) {
  return (
    <svg viewBox="0 0 20 20" className={className} fill="none" aria-hidden="true">
      <path d="M4.2 8.1h2.2L9.6 5.4v9.2L6.4 11.9H4.2V8.1Z" fill="currentColor" />
      {muted ? (
        <path d="M12.2 8.2 16.4 12.4M16.4 8.2 12.2 12.4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      ) : (
        <path d="M12.4 7.4a3.4 3.4 0 0 1 0 5.2M14.4 5.6a6 6 0 0 1 0 8.8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      )}
    </svg>
  )
}
