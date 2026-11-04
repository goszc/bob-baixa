import type { Lang } from "./types"

export function isTime(value: string): boolean {
  const trimmed = value.trim()
  return /^(\d{1,2}:)?\d{1,2}:\d{2}$/.test(trimmed) || /^\d+$/.test(trimmed)
}

export function extractUrls(text: string): string[] {
  const found = text.match(/https?:\/\/[^\s<>"']+/gi) ?? []
  const cleaned = found.map((url) => url.replace(/[),.;]+$/g, ""))
  return [...new Set(cleaned)]
}

export function formatBytes(bytes: number, lang: Lang): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—"
  const units = ["b", "kb", "mb", "gb"]
  let value = bytes
  let index = 0
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024
    index += 1
  }
  const digits = value >= 10 || index === 0 ? 0 : 1
  const locale = lang === "pt" ? "pt-BR" : "en-US"
  return `${value.toLocaleString(locale, { maximumFractionDigits: digits, minimumFractionDigits: digits })} ${units[index]}`
}

export function formatSpeed(bytesPerSec: number | undefined, lang: Lang): string {
  if (!bytesPerSec || bytesPerSec <= 0) return "—"
  return `${formatBytes(bytesPerSec, lang)}/s`
}

export function formatClock(seconds: number | undefined): string {
  if (seconds === undefined || !Number.isFinite(seconds) || seconds < 0) return "—"
  const total = Math.round(seconds)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const secs = total % 60
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`
  }
  return `${minutes}:${String(secs).padStart(2, "0")}`
}

export function formatWhen(iso: string, lang: Lang): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleString(lang === "pt" ? "pt-BR" : "en-US", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  })
}

export function nextIsoFromClock(hhmm: string): string | null {
  const match = /^(\d{2}):(\d{2})$/.exec(hhmm)
  if (!match) return null
  const hours = Number(match[1])
  const minutes = Number(match[2])
  if (hours > 23 || minutes > 59) return null
  const now = new Date()
  const target = new Date(now)
  target.setSeconds(0, 0)
  target.setHours(hours, minutes, 0, 0)
  if (target.getTime() <= now.getTime()) target.setDate(target.getDate() + 1)
  return target.toISOString()
}

export function isMediaPath(path: string): boolean {
  return /\.(mp4|mkv|webm|mov|avi|m4v|mp3|m4a|wav|flac|opus|ogg)$/i.test(path)
}

export function isTextPath(path: string): boolean {
  return /\.(txt|list|urls)$/i.test(path)
}

export function fileName(path: string): string {
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}
