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
import { liveStats } from "../lib/stats"
import { tracksForPaths, useMusic } from "../lib/music"
import { playTracks } from "../lib/player"
import { trackKey, type Track } from "../lib/library"
import { useLibraryTracks } from "../lib/use-library"
import { weekPaths } from "../lib/week"
import type { Key } from "../lib/i18n"
import { builtinPresets, configMatchesPreset } from "../lib/presets"
import type { Quality } from "../lib/types"
import { cn } from "../lib/cn"
import {
  acceptClipboard,
  analyze,
  appendLinks,
  applyPreset,
  deletePreset,
  dismissClipboard,
  dismissAllAnalyzed,
  dismissAnalyzed,
  downloadNow,
  go,
  renamePreset,
  savePreset,
  setConfig,
  setDraft,
  setField,
  toggleAllEntries,
  toggleEntry,
  updatePreset,
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
  const music = useMusic()
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

  const library = useLibraryTracks()
  const recent = music.recent
    .map((item) => library.find((track) => trackKey(track.path) === trackKey(item.path)))
    .filter((track): track is Track => Boolean(track))
    .slice(0, 6)
  const weekly = tracksForPaths(weekPaths(music.events), library)

  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_home")}</h1>
      <section className="mt-8">
        <h2 className="text-[22px] font-medium tracking-[-0.03em]">{t("home_continue")}</h2>
        {recent.length === 0 ? (
          <div className="mt-4">
            <p className="max-w-[360px] text-[16px] leading-relaxed">{t("empty_library")}</p>
            <button type="button" className="mt-4 h-10 rounded-full px-4 text-[14px]" style={{ background: "var(--accent)", color: "#101010" }} onClick={() => go("discover")}>
              {t("empty_action")}
            </button>
          </div>
        ) : (
          <div className="home-row mt-4">
            {recent.map((track, index) => (
              <button key={track.path} type="button" className="rounded-2xl bg-soft p-3 text-left" onClick={() => playTracks(recent, index)}>
                <Thumb src={track.thumbnail} videoUrl={track.pageUrl || track.title} className="aspect-square w-full rounded-xl" />
                <span className="mt-3 block truncate text-[14px]">{track.title}</span>
                <span className="block truncate text-[12px] text-muted">{track.artist}</span>
              </button>
            ))}
          </div>
        )}
      </section>
      {weekly.length ? (
        <section className="mt-10">
          <h2 className="text-[22px] font-medium tracking-[-0.03em]">{t("home_weekly")}</h2>
          <div className="home-grid mt-4">
            <button type="button" className="flex min-h-[180px] flex-col justify-between rounded-2xl p-4 text-left" style={{ background: "color-mix(in srgb, var(--accent) 32%, var(--surface))" }} onClick={() => playTracks(weekly, 0)}>
              <span className="text-[18px] font-medium tracking-[-0.03em]">{t("home_weekly")}</span>
              <span className="text-[13px] text-muted">{weekly.length}</span>
            </button>
            {weekly.slice(0, 5).map((track, index) => (
              <button key={track.path} type="button" className="rounded-2xl bg-soft p-3 text-left" onClick={() => playTracks(weekly, index)}>
                <Thumb src={track.thumbnail} videoUrl={track.pageUrl || track.title} className="aspect-square w-full rounded-xl" />
                <span className="mt-3 block truncate text-[14px]">{track.title}</span>
                <span className="block truncate text-[12px] text-muted">{track.artist}</span>
              </button>
            ))}
          </div>
        </section>
      ) : null}
      <div className="mt-10">
      <PageHeader title={t("download_title")} sub={t("download_sub")} />
      <LiveCard />
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
      <Presets />
      {app.error && app.page === "home" ? <p className="mt-3 text-[13px] text-danger">{app.error}</p> : null}
      {app.analyzed.length > 0 ? (
        <div className="mt-4 flex justify-end">
          <button type="button" className="text-[13px] text-muted hover:text-ink" onClick={dismissAllAnalyzed}>
            {t("close_all")}
          </button>
        </div>
      ) : null}
      <div className="mt-3 space-y-3">
        {app.analyzed.map((media) =>
          media.kind === "video" ? (
            <article key={media.id} className="flex gap-4 rounded-2xl border border-line bg-surface p-3">
              <Thumb src={media.thumbnail} videoUrl={media.url} className="h-[104px] w-[184px]" />
              <div className="min-w-0 flex-1 py-1">
                <div className="flex items-start gap-3">
                  <h2 className="min-w-0 flex-1 truncate text-[16px] font-medium tracking-tight select-text">{media.title}</h2>
                  <button type="button" className="text-[13px] text-muted hover:text-ink" aria-label={t("close")} onClick={() => dismissAnalyzed(media.id)}>
                    {t("close")}
                  </button>
                </div>
                <p className="mt-1 text-[13px] text-muted">
                  {[media.channel, media.duration ? formatClock(media.duration) : null].filter(Boolean).join(" · ")}
                </p>
              </div>
            </article>
          ) : (
            <article key={media.id} className="rounded-2xl border border-line bg-surface p-3">
              <div className="flex items-center justify-between gap-3 px-1">
                <div className="min-w-0">
                  <h2 className="truncate text-[16px] font-medium tracking-tight">{media.title}</h2>
                  <p className="mt-1 text-[13px] text-muted">
                    {media.entries.length} {t("videos")}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-3">
                  <button type="button" className="text-[13px] text-muted hover:text-ink" onClick={() => toggleAllEntries(media.id, (app.entryChoice[media.id] ?? []).length !== media.entries.length)}>
                    {t("select_all")}
                  </button>
                  <button type="button" className="text-[13px] text-muted hover:text-ink" aria-label={t("close")} onClick={() => dismissAnalyzed(media.id)}>
                    {t("close")}
                  </button>
                </div>
              </div>
              <ul className="mt-3 max-h-64 overflow-auto">
                {media.entries.map((entry) => {
                  const selected = (app.entryChoice[media.id] ?? []).includes(entry.id)
                  return (
                    <li key={entry.id} className="flex items-center gap-3 border-t border-line py-2.5">
                      <input type="checkbox" checked={selected} aria-label={entry.title} onChange={() => toggleEntry(media.id, entry.id)} className="h-4 w-4 accent-[var(--ink)]" />
                      <Thumb src={entry.thumbnail} videoUrl={entry.url} className="h-12 w-[84px] rounded-lg" />
                      <span className="min-w-0 flex-1 truncate text-[14px]">{entry.title}</span>
                      <span className="text-[12px] tabular-nums text-muted">{formatClock(entry.duration)}</span>
                    </li>
                  )
                })}
              </ul>
              {media.unmatched?.length ? (
                <div className="mt-3 border-t border-line px-1 pt-3 text-[13px] text-muted">
                  <p>{t("import_unmatched")}</p>
                  <ul className="mt-1 space-y-1">
                    {media.unmatched.map((name) => (
                      <li key={name}>{name}</li>
                    ))}
                  </ul>
                </div>
              ) : null}
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
                {["320k", "256k", "192k", "128k", "96k", "32k", "24k"].map((rate) => (
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
    </div>
  )
}

function Presets() {
  const app = useApp()
  const t = useT()
  const [name, setName] = useState("")
  const [selected, setSelected] = useState<string | null>(null)
  const all = [...builtinPresets, ...app.config.presets]
  const matched = all.find((preset) => configMatchesPreset(app.config, preset))
  const activeId = selected ?? matched?.id
  const custom = app.config.presets.find((preset) => preset.id === selected) ?? null
  const typed = name.trim()
  return (
    <div className="mt-5">
      <div className="text-[12px] text-muted">{t("presets")}</div>
      <p className="mt-1 text-[13px] leading-relaxed text-muted">{t("preset_hint")}</p>
      <div className="mt-3 flex flex-wrap gap-2">
        {all.map((preset) => {
          const on = activeId === preset.id
          return (
            <button
              key={preset.id}
              type="button"
              onClick={() => {
                setSelected(preset.id)
                applyPreset(preset)
              }}
              className={cn(
                "h-8 rounded-full border px-3 text-[13px] transition-colors",
                on ? "border-fill bg-fill text-onfill" : "border-line text-muted hover:text-ink",
              )}
            >
              {preset.id === "builtin-economy" ? t("preset_economy") : preset.name}
            </button>
          )
        })}
      </div>
      {activeId === "builtin-economy" ? <p className="mt-3 max-w-xl text-[13px] leading-relaxed text-muted">{t("economy_hint")}</p> : null}
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Input className="w-[200px]" value={name} placeholder={t("preset_name")} onChange={(event) => setName(event.target.value)} />
        <Button variant="outline" onClick={() => savePreset(name)}>
          {t("preset_save")}
        </Button>
        {custom && typed && typed !== custom.name ? (
          <Button
            variant="ghost"
            onClick={() => {
              renamePreset(custom.id, typed)
              setName("")
            }}
          >
            {t("preset_rename")}
          </Button>
        ) : null}
        {custom ? (
          <Button variant="ghost" onClick={() => updatePreset(custom.id)}>
            {t("preset_update")}
          </Button>
        ) : null}
        {custom ? (
          <Button
            variant="ghost"
            onClick={() => {
              deletePreset(custom.id)
              setSelected(null)
            }}
          >
            {t("preset_delete")}
          </Button>
        ) : null}
      </div>
    </div>
  )
}

function LiveCard() {
  const t = useT()
  const music = useMusic()
  const stats = liveStats(music.events, Date.now())
  const hour = stats.hour == null ? "—" : `${String(stats.hour).padStart(2, "0")}h`
  const cards = [
    [t("home_week"), `${Math.round(stats.weekMinutes)} min`],
    [t("home_month_artist"), stats.artist || "—"],
    [t("home_hour"), hour],
  ]
  return (
    <div className="mb-8 grid grid-cols-3 gap-3">
      {cards.map(([label, value]) => (
        <div key={label} className="rounded-2xl bg-surface px-4 py-4">
          <div className="text-[12px] text-muted">{label}</div>
          <div className="mt-1 truncate text-[18px] tracking-[-0.03em]">{value}</div>
        </div>
      ))}
    </div>
  )
}
