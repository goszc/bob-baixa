import type { SelectHTMLAttributes } from "react"
import { cn } from "../../lib/cn"

export function Select({ className, children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className="relative">
      <select
        className={cn(
          "h-10 w-full appearance-none rounded-[10px] border border-line bg-surface px-3 pr-8 text-[14px] outline-none transition-colors focus:border-ink/40",
          className,
        )}
        {...props}
      >
        {children}
      </select>
      <svg
        viewBox="0 0 12 12"
        aria-hidden="true"
        className="pointer-events-none absolute right-3 top-1/2 h-3 w-3 -translate-y-1/2 text-muted"
      >
        <path d="M2 4.2 L6 8 L10 4.2" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      </svg>
    </div>
  )
}
