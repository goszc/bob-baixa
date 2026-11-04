import { useId } from "react"
import { cn } from "../lib/cn"

export function Logo({ className }: { className?: string }) {
  const maskId = useId()
  return (
    <svg viewBox="0 0 28 20" className={cn("text-ink", className)} aria-hidden="true">
      <mask id={maskId}>
        <rect width="28" height="20" rx="4.6" fill="white" />
        <path d="M11.1 5.15 L11.1 14.85 L20.4 10 Z" fill="black" />
      </mask>
      <rect width="28" height="20" rx="4.6" fill="currentColor" mask={`url(#${maskId})`} />
    </svg>
  )
}
