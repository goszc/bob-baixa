import { useEffect, useRef, useState, type ReactNode } from "react"

export function VirtualList({
  count,
  rowHeight,
  className,
  render,
}: {
  count: number
  rowHeight: number
  className?: string
  render: (index: number) => ReactNode
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [scroll, setScroll] = useState(0)
  const [height, setHeight] = useState(0)

  useEffect(() => {
    const node = ref.current
    if (!node) return
    const measure = () => setHeight(node.clientHeight)
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(node)
    return () => observer.disconnect()
  }, [])

  const overscan = 8
  const start = Math.max(0, Math.floor(scroll / rowHeight) - overscan)
  const end = Math.min(count, start + Math.ceil(height / rowHeight) + overscan * 2)
  const rows = []
  for (let index = start; index < end; index += 1) rows.push(index)

  return (
    <div ref={ref} className={className} onScroll={(event) => setScroll(event.currentTarget.scrollTop)}>
      <div style={{ height: count * rowHeight, position: "relative" }}>
        {rows.map((index) => (
          <div key={index} style={{ position: "absolute", top: index * rowHeight, left: 0, right: 0, height: rowHeight }}>
            {render(index)}
          </div>
        ))}
      </div>
    </div>
  )
}
