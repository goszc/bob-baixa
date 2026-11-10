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

export function thumbSources(src?: string, videoUrl?: string): string[] {
  const sources: string[] = []
  const add = (url?: string) => {
    if (url && (url.startsWith("https://") || url.startsWith("http://127.0.0.1:") || url.startsWith("http://localhost:") || (url.startsWith("/") && !url.startsWith("//"))) && !sources.includes(url)) sources.push(url)
  }
  add(src)
  add(youtubeThumb(videoUrl))
  add(youtubeThumb(src))
  add(youtubeThumb(videoUrl, "mqdefault"))
  add(youtubeThumb(src, "mqdefault"))
  return sources
}
