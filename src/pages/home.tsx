import { useEffect, useState, type DragEvent } from "react"
import { Field } from "../components/field"
import { PageHeader } from "../components/page-header"
import { Thumb } from "../components/thumb"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { Select } from "../components/ui/select"
import { Switch } from "../components/ui/switch"
import { Textarea } from "../components/ui/textarea"
import { formatClock } from "../lib/format"
import type { Key } from "../lib/i18n"
import type { Quality } from "../lib/types"
import { cn } from "../lib/cn"
import {
  acceptClipboard,
  analyze,
  appendLinks,
  dismissClipboard,
  downloadNow,
  setConfig,
  setDraft,
  setField,
  toggleAllEntries,
  toggleEntry,
  useApp,
  useT,
} from "../store"

const QUALITIES: { value: Quality; key: Key }[] = [
  { value: "best", key: "best" },
  { value: "2160", key: "p2160" },
  { value: "1440", key: "p1440" },
  { value: "1080", key: "p1080" },
  { value: "720", key: "p720" },
  { value: "480", key: "p480" },
  { value: "360", key: "p360" },
]

export function Home() {
  const app = useApp()
  const t = useT()
  const [over, setOver] = useState(false)
  const [more, setMore] = useState(false)
  const dragging = over || app.dragging
  const toolsReady = app.tools.ytdlp === "ready" && app.tools.ffmpeg === "ready"

  useEffect(() => {
    const handle = window.setTimeout(() => {
      void analyze()
    }, 400)
    return () => window.clearTimeout(handle)
  }, [app.draft])

  async function onDrop(event: DragEvent) {
    event.preventDefault()
    setOver(false)
    const file = Array.from(event.dataTransfer.files).find((item) => item.name.endsWith(".txt") || item.type.startsWith("text"))
    if (file) {
      await appendLinks(await file.text())
      return
    }
    const text = event.dataTransfer.getData("text/uri-list") || event.dataTransfer.getData("text/plain")
    if (text) await appendLinks(text)
  }

  return (
    <div>
      <PageHeader title={t("download_title")} sub={t("download_sub")} />
      {!toolsReady ? (
        <div className="mb-6">
          <div className="flex items-center justify-between text-[13px] text-muted">
            <span>{t("tools_boot")}</span>
            <span className="tabular-nums">{Math.round((app.tools.progress ?? 0.08) * 100)}%</span>
          </div>
          <div className="mt-2 h-[3px] overflow-hidden rounded-full bg-soft">
            <div className="h-full rounded-full bg-ink" style={{ width: `${Math.max(8, (app.tools.progress ?? 0.08) * 100)}%` }} />
          </div>
        </div>
      ) : null}
      {app.clipboardUrl ? (
        <div className="mb-4 flex items-center gap-3 rounded-2xl border border-line bg-surface px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-[13px] text-muted">{t("clipboard_found")}</div>
            <div className="truncate text-[14px]">{app.clipboardUrl}</div>
          </div>
          <Button size="sm" variant="ghost" onClick={dismissClipboard}>
            {t("clipboard_dismiss")}
          </Button>
          <Button size="sm" onClick={acceptClipboard}>
            {t("clipboard_add")}
          </Button>
        </div>
      ) : null}
      <div
        onDragEnter={(event) => {
          event.preventDefault()
          setOver(true)
        }}
        onDragOver={(event) => {
          event.preventDefault()
          setOver(true)
        }}
        onDragLeave={() => setOver(false)}
        onDrop={(event) => void onDrop(event)}
        className={cn("rounded-2xl border bg-surface p-4 transition-colors", dragging ? "border-ink" : "border-line")}
      >
        <Textarea
          value={app.draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder={t("placeholder_urls")}
          className="min-h-[88px]"
          spellCheck={false}
          onKeyDown={(event) => {
            if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
              event.preventDefault()
              void downloadNow()
            }
          }}
        />
        <div className="mt-3 flex items-center justify-between gap-3">
          <span className="text-[12px] text-muted">{app.busy === "analyze" ? t("reading") : dragging ? t("dropping") : t("drop_hint")}</span>
          <Button disabled={app.busy === "analyze"} onClick={() => void downloadNow()}>
            {t("download_now")}
          </Button>
        </div>
      </div>
      {app.error && app.page === "home" ? <p className="mt-3 text-[13px] text-danger">{app.error}</p> : null}
      <div className="mt-5 space-y-3">
        {app.analyzed.map((media) =>
          media.kind === "video" ? (
            <article key={media.id} className="flex gap-4 rounded-2xl border border-line bg-surface p-3">
              <Thumb src={media.thumbnail} className="h-[104px] w-[184px]" />
              <div className="min-w-0 flex-1 py-1">
                <h2 className="truncate text-[16px] font-medium tracking-tight select-text">{media.title}</h2>
                <p className="mt-1 text-[13px] text-muted">
                  {[media.channel, media.duration ? formatClock(media.duration) : null].filter(Boolean).join(" · ")}
                </p>
              </div>
            </article>
          ) : (
            <article key={media.id} className="rounded-2xl border border-line bg-surface p-3">
              <div className="flex items-center justify-between gap-3 px-1">
                <div>
                  <h2 className="text-[16px] font-medium tracking-tight">{media.title}</h2>
                  <p className="mt-1 text-[13px] text-muted">
                    {media.entries.length} {t("videos")}
                  </p>
                </div>
                <button type="button" className="text-[13px] text-muted" onClick={() => toggleAllEntries(media.id, (app.entryChoice[media.id] ?? []).length !== media.entries.length)}>
                  {t("select_all")}
                </button>
              </div>
              <ul className="mt-3 max-h-64 overflow-auto">
                {media.entries.map((entry) => {
                  const selected = (app.entryChoice[media.id] ?? []).includes(entry.id)
                  return (
                    <li key={entry.id} className="flex items-center gap-3 border-t border-line py-2.5">
                      <input type="checkbox" checked={selected} aria-label={entry.title} onChange={() => toggleEntry(media.id, entry.id)} className="h-4 w-4 accent-[var(--ink)]" />
                      <Thumb src={entry.thumbnail} className="h-12 w-[84px] rounded-lg" />
                      <span className="min-w-0 flex-1 truncate text-[14px]">{entry.title}</span>
                      <span className="text-[12px] tabular-nums text-muted">{formatClock(entry.duration)}</span>
                    </li>
                  )
                })}
              </ul>
            </article>
          ),
        )}
      </div>
      <button type="button" className="mt-6 text-[13px] text-muted hover:text-ink" onClick={() => setMore((open) => !open)}>
        {more ? t("less") : t("more")}
      </button>
      {more ? (
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <Field label={t("quality")}>
            <Select value={app.config.quality} onChange={(event) => setConfig({ quality: event.target.value as Quality })}>
              {QUALITIES.map((item) => (
                <option key={item.value} value={item.value}>
                  {t(item.key)}
                </option>
              ))}
            </Select>
          </Field>
          {app.config.audioOnly ? (
            <Field label={t("audio_format")}>
              <Select value={app.config.audioFormat} onChange={(event) => setConfig({ audioFormat: event.target.value as "mp3" })}>
                {["mp3", "m4a", "opus", "flac"].map((format) => (
                  <option key={format} value={format}>
                    {format}
                  </option>
                ))}
              </Select>
            </Field>
          ) : (
            <Field label={t("container")}>
              <Select value={app.config.container} onChange={(event) => setConfig({ container: event.target.value as "mp4" })}>
                {["mp4", "mkv", "webm"].map((format) => (
                  <option key={format} value={format}>
                    {format}
                  </option>
                ))}
              </Select>
            </Field>
          )}
          <div className="flex items-center justify-between sm:col-span-2">
            <span className="text-[14px]">{t("audio_only")}</span>
            <Switch label={t("audio_only")} checked={app.config.audioOnly} onCheckedChange={(value) => setConfig({ audioOnly: value })} />
          </div>
          {app.config.audioOnly ? (
            <Field label={t("bitrate")}>
              <Select value={app.config.audioBitrate} disabled={app.config.audioFormat === "flac"} onChange={(event) => setConfig({ audioBitrate: event.target.value })}>
                {["320k", "256k", "192k", "128k", "96k"].map((rate) => (
                  <option key={rate} value={rate}>
                    {rate}
                  </option>
                ))}
              </Select>
            </Field>
          ) : null}
          <Field label={t("clip_start")}>
            <Input value={app.clipStart} placeholder="1:20" onChange={(event) => setField("clipStart", event.target.value)} />
          </Field>
          <Field label={t("clip_end")}>
            <Input value={app.clipEnd} placeholder="3:45" onChange={(event) => setField("clipEnd", event.target.value)} />
          </Field>
          <Field label={t("speed_this")}>
            <Input value={app.itemSpeed} placeholder="2m" onChange={(event) => setField("itemSpeed", event.target.value)} />
          </Field>
        </div>
      ) : null}
    </div>
  )
}
