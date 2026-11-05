import { useRef } from "react"

export function Scrubber({
  value,
  max,
  label,
  onChange,
  disabled,
}: {
  value: number
  max: number
  label: string
  onChange: (value: number) => void
  disabled?: boolean
}) {
  const ref = useRef<HTMLDivElement>(null)
  const span = max > 0 ? max : 0
  const ratio = span > 0 ? Math.min(1, Math.max(0, value / span)) : 0

  function at(clientX: number) {
    const box = ref.current?.getBoundingClientRect()
    if (!box || span <= 0) return
    const next = Math.min(1, Math.max(0, (clientX - box.left) / box.width)) * span
    onChange(next)
  }

  return (
    <div
      ref={ref}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={Math.round(span)}
      aria-valuenow={Math.round(Math.min(value, span || value))}
      aria-disabled={disabled || undefined}
      className="group relative flex h-5 cursor-pointer items-center outline-none focus-visible:ring-2 focus-visible:ring-ink/25"
      onPointerDown={(event) => {
        if (disabled) return
        event.currentTarget.setPointerCapture(event.pointerId)
        at(event.clientX)
      }}
      onPointerMove={(event) => {
        if (disabled || !event.currentTarget.hasPointerCapture(event.pointerId)) return
        at(event.clientX)
      }}
      onKeyDown={(event) => {
        if (disabled || span <= 0) return
        const step = span <= 1 ? (event.shiftKey ? 0.1 : 0.05) : event.shiftKey ? 10 : 5
        if (event.key === "ArrowRight") {
          event.preventDefault()
          onChange(Math.min(span, value + step))
        }
        if (event.key === "ArrowLeft") {
          event.preventDefault()
          onChange(Math.max(0, value - step))
        }
      }}
    >
      <div className="relative h-[3px] w-full rounded-full bg-soft">
        <div className="absolute inset-y-0 left-0 rounded-full bg-fill" style={{ width: `${ratio * 100}%` }} />
        <div className="absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fill" style={{ left: `${ratio * 100}%` }} />
      </div>
    </div>
  )
}
