import type { ReactNode } from "react"

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[12px] text-muted">{label}</span>
      {children}
      {hint ? <span className="mt-1.5 block text-[12px] leading-relaxed text-muted">{hint}</span> : null}
    </label>
  )
}
