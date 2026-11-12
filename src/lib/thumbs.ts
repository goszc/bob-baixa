const ID = /(?:v=|youtu\.be\/|\/shorts\/|\/embed\/|\/vi\/)([\w-]{11})/
const BRACKET = /\[([\w-]{11})\]/

export function youtubeId(value?: string): string | undefined {
  if (!value) return undefined
  if (/^[\w-]{11}$/.test(value)) return value
  return value.match(ID)?.[1] ?? value.match(BRACKET)?.[1]
}

export function youtubeThumb(value?: string, quality: "hqdefault" | "mqdefault" = "hqdefault"): string | undefined {
  const id = youtubeId(value)
  return id ? `https://i.ytimg.com/vi/${id}/${quality}.jpg` : undefined
}

export function coverMark(seed?: string): string {
  const label = seed?.trim() || "bob"
  let hash = 0
  for (const ch of label) hash = (hash * 33 + ch.charCodeAt(0)) >>> 0
  const hue = [18, 28, 152, 188, 210, 262, 330, 48][hash % 8]
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80"><rect width="80" height="80" fill="hsl(${hue} 34% 42%)"/><path d="M34 56V24l22-6v28" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round"/><circle cx="30" cy="56" r="6" fill="#fff"/><circle cx="52" cy="46" r="6" fill="#fff"/></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

export function thumbSources(src?: string, videoUrl?: string): string[] {
  const sources: string[] = []
  const add = (url?: string) => {
    if (url && (url.startsWith("https://") || url.startsWith("data:image/") || url.startsWith("http://127.0.0.1:") || url.startsWith("http://localhost:") || (url.startsWith("/") && !url.startsWith("//"))) && !sources.includes(url)) sources.push(url)
  }
  add(src)
  add(youtubeThumb(videoUrl))
  add(youtubeThumb(src))
  add(youtubeThumb(videoUrl, "mqdefault"))
  add(youtubeThumb(src, "mqdefault"))
  return sources
}
