import { isTauri } from "./api"

const memory = new Map<string, string>()

export async function resolveThumb(url?: string): Promise<string | undefined> {
  if (!url) return undefined
  if (!url.startsWith("http://") && !url.startsWith("https://")) return url
  const cached = memory.get(url)
  if (cached) return cached
  if (!isTauri()) {
    memory.set(url, url)
    return url
  }
  const { invoke, convertFileSrc } = await import("@tauri-apps/api/core")
  const path = await invoke<string>("cache_thumb", { url })
  const src = convertFileSrc(path)
  memory.set(url, src)
  return src
}
