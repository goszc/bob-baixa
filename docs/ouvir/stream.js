/** Same-origin media URL helpers. The service worker adds the Drive bearer token. */

export function fileIdFrom(pathname) {
  const parts = String(pathname || "")
    .split("/")
    .filter(Boolean)
  const at = parts.lastIndexOf("stream")
  if (at < 0 || at + 1 >= parts.length) return ""
  const id = parts[at + 1]
  return /^[A-Za-z0-9_-]+$/.test(id) ? id : ""
}

export function parseByteRange(header, size) {
  const match = /^bytes=(\d*)-(\d*)$/i.exec(String(header || "").trim())
  if (!match || size <= 0) return null
  if (!match[1] && !match[2]) return null
  if (!match[1]) {
    const length = Math.min(size, Number(match[2]))
    return { start: size - length, end: size - 1 }
  }
  const start = Number(match[1])
  if (!Number.isFinite(start) || start >= size) return null
  const end = match[2] ? Math.min(size - 1, Number(match[2])) : size - 1
  if (!Number.isFinite(end) || end < start) return null
  return { start, end }
}

/**
 * Build a same-origin media response. A ranged Drive reply stays 206 so the
 * element can seek without taking the whole file into memory.
 */
export function mediaResponseInit(upstreamStatus, upstreamHeaders, requestRange, size) {
  const headers = new Headers()
  const rawType = upstreamHeaders.get("content-type") || "application/octet-stream"
  headers.set("Content-Type", rawType.split(";")[0])
  headers.set("Accept-Ranges", "bytes")
  headers.set("Cache-Control", "private, no-store")
  const exposedRange = upstreamHeaders.get("content-range")
  const exposedLength = upstreamHeaders.get("content-length")
  let status = upstreamStatus
  if (upstreamStatus === 206 || exposedRange) {
    status = 206
    if (exposedRange) headers.set("Content-Range", exposedRange)
    else {
      const parsed = parseByteRange(requestRange, size)
      if (parsed) headers.set("Content-Range", `bytes ${parsed.start}-${parsed.end}/${size}`)
    }
  }
  if (exposedLength) headers.set("Content-Length", exposedLength)
  else if (status === 206 && headers.has("Content-Range")) {
    const parsed = parseByteRange(requestRange, size)
    if (parsed) headers.set("Content-Length", String(parsed.end - parsed.start + 1))
  } else if (status === 200 && size > 0) {
    headers.set("Content-Length", String(size))
  }
  return { status, headers }
}
