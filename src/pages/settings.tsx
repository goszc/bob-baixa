import { useEffect, useState, type ReactNode } from "react"
import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { Select } from "../components/ui/select"
import { Switch } from "../components/ui/switch"
import type { Key } from "../lib/i18n"
import { cn } from "../lib/cn"
import { invoke, isTauri } from "../lib/api"
import type { CookiesBrowser, Lang, ThemeMode } from "../lib/types"
import { checkUpdates, makeDiagnostic, measureLibrary, openNotes, pickFolder, reinstallTools, setConfig, updateYtDlp, useApp, useT } from "../store"

const CATS: { id: string; key: Key }[] = [
  { id: "sponsor", key: "cat_sponsor" },
  { id: "intro", key: "cat_intro" },
  { id: "outro", key: "cat_outro" },
  { id: "selfpromo", key: "cat_selfpromo" },
  { id: "interaction", key: "cat_interaction" },
  { id: "preview", key: "cat_preview" },
  { id: "filler", key: "cat_filler" },
  { id: "music_offtopic", key: "cat_music_offtopic" },
]

const BROWSERS: CookiesBrowser[] = ["auto", "none", "chrome", "edge", "firefox", "brave", "opera", "vivaldi", "chromium"]

export function Settings() {
  const app = useApp()
  const t = useT()
  const config = app.config
  return (
    <div>
      <PageHeader title={t("settings_title")} sub={t("settings_sub")} />
      <Section title={t("appearance")} first>
        <div className="border-b border-line py-4">
          <div className="flex items-start justify-between gap-6">
            <div className="max-w-[240px]">
              <div className="text-[14px]">{t("theme")}</div>
              <p className="mt-1 text-[13px] leading-relaxed text-muted">{t("theme_hint")}</p>
            </div>
            <ThemePicker theme={config.theme} onPick={(theme) => setConfig({ theme })} systemLabel={t("theme_system")} />
          </div>
        </div>
        <Row label={t("language")} hint={t("language_hint")}>
          <Segmented<Lang>
            value={config.language}
            onChange={(language) => setConfig({ language })}
            options={[
              { value: "pt", label: t("portuguese") },
              { value: "en", label: t("english") },
            ]}
          />
        </Row>
      </Section>
      <Section title={t("destination")}>
        <Row label={t("folder")}>
          <div className="flex gap-2">
            <Input readOnly className="w-[240px]" value={config.outputDir} placeholder="Downloads/bob baixa" />
            <Button variant="outline" onClick={() => void pickFolder()}>
              {t("choose_folder")}
            </Button>
          </div>
        </Row>
        <div className="border-b border-line py-4">
          <div className="text-[14px]">{t("filename")}</div>
          <p className="mt-1 text-[13px] text-muted">{t("filename_hint")}</p>
          <Input className="mt-3" value={config.filenameTemplate} onChange={(event) => setConfig({ filenameTemplate: event.target.value })} />
        </div>
      </Section>
      <Section title={t("queue_settings")}>
        <Row label={t("parallel")} hint={t("parallel_hint")}>
          <Segmented
            value={String(config.parallel)}
            onChange={(value) => setConfig({ parallel: Number(value) })}
            options={[1, 2, 3, 4].map((count) => ({ value: String(count), label: String(count) }))}
          />
        </Row>
        <Row label={t("global_speed")}>
          <Input className="w-[120px]" placeholder="2m" value={config.globalSpeed} onChange={(event) => setConfig({ globalSpeed: event.target.value })} />
        </Row>
        <Row label={t("clipboard_watch")} hint={t("clipboard_hint")}>
          <Switch label={t("clipboard_watch")} checked={config.clipboardWatch} onCheckedChange={(value) => setConfig({ clipboardWatch: value })} />
        </Row>
        <Row label={t("tray")} hint={t("tray_hint")}>
          <Switch label={t("tray")} checked={config.minimizeToTray} onCheckedChange={(value) => setConfig({ minimizeToTray: value })} />
        </Row>
        <Row label={t("notify")}>
          <Switch label={t("notify")} checked={config.notifications} onCheckedChange={(value) => setConfig({ notifications: value })} />
        </Row>
        <Row label={t("hotkey")} hint={t("hotkey_help")}>
          <Switch label={t("hotkey")} checked={config.hotkey} onCheckedChange={(value) => setConfig({ hotkey: value })} />
        </Row>
        <Row label={t("context_menu")} hint={t("context_menu_hint")}>
          <Switch label={t("context_menu")} checked={config.contextMenu} onCheckedChange={(value) => setConfig({ contextMenu: value })} />
        </Row>
      </Section>
      <Section title={t("subs_extras")}>
        <Row label={t("subs")}>
          <Switch label={t("subs")} checked={config.subtitles} onCheckedChange={(value) => setConfig({ subtitles: value })} />
        </Row>
        <Row label={t("embed_subs")}>
          <Switch label={t("embed_subs")} checked={config.embedSubs} onCheckedChange={(value) => setConfig({ embedSubs: value })} />
        </Row>
        <Row label={t("sub_langs")}>
          <Input className="w-[180px]" value={config.subLangs} onChange={(event) => setConfig({ subLangs: event.target.value })} />
        </Row>
        <Row label={t("embed_thumb")}>
          <Switch label={t("embed_thumb")} checked={config.embedThumbnail} onCheckedChange={(value) => setConfig({ embedThumbnail: value })} />
        </Row>
        <Row label={t("embed_meta")}>
          <Switch label={t("embed_meta")} checked={config.embedMetadata} onCheckedChange={(value) => setConfig({ embedMetadata: value })} />
        </Row>
        <Row label={t("embed_chapters")}>
          <Switch label={t("embed_chapters")} checked={config.embedChapters} onCheckedChange={(value) => setConfig({ embedChapters: value })} />
        </Row>
      </Section>
      <Section title={t("sponsor")}>
        <Row label={t("sponsor")} hint={t("sponsor_help")}>
          <Switch label={t("sponsor")} checked={config.sponsorblock} onCheckedChange={(value) => setConfig({ sponsorblock: value })} />
        </Row>
        <div className="flex flex-wrap gap-2 py-4">
          {CATS.map((cat) => {
            const on = config.sponsorCategories.includes(cat.id)
            return (
              <button
                key={cat.id}
                type="button"
                onClick={() =>
                  setConfig({
                    sponsorCategories: on ? config.sponsorCategories.filter((item) => item !== cat.id) : [...config.sponsorCategories, cat.id],
                  })
                }
                className={cn(
                  "h-8 rounded-full border px-3 text-[13px] transition-colors",
                  on ? "border-fill bg-fill text-onfill" : "border-line text-muted hover:text-ink",
                )}
              >
                {t(cat.key)}
              </button>
            )
          })}
        </div>
      </Section>
      <Section title={t("cookies")}>
        <Row label={t("cookies")} hint={t("cookies_help")}>
          <div className="w-[180px]">
            <Select value={config.cookiesBrowser} onChange={(event) => setConfig({ cookiesBrowser: event.target.value as CookiesBrowser })}>
              {BROWSERS.map((browser) => (
                <option key={browser} value={browser}>
                  {browser === "none" ? t("none") : browser === "auto" ? t("auto") : browser}
                </option>
              ))}
            </Select>
          </div>
        </Row>
      </Section>
      <Section title={t("tools")}>
        <p className="py-3 text-[13px] leading-relaxed text-muted">{t("tools_hint")}</p>
        <p className="pb-3 text-[13px] text-muted">
          bob baixa · {app.tools.appVersion ?? "0.6.2"}
          <br />
          yt-dlp {app.tools.ytdlpVersion ? `· ${app.tools.ytdlpVersion}` : `· ${t("tools_missing")}`}
          <br />
          ffmpeg {app.tools.ffmpegVersion ? `· ${app.tools.ffmpegVersion}` : `· ${t("tools_missing")}`}
        </p>
        <Row label={t("auto_update")} hint={t("auto_update_hint")}>
          <Switch label={t("auto_update")} checked={config.autoUpdate} onCheckedChange={(value) => setConfig({ autoUpdate: value })} />
        </Row>
        <div className="border-b border-line py-4">
          <div className="text-[14px]">{t("update_url")}</div>
          <Input className="mt-3" value={config.updateUrl} placeholder="https://.../latest.json" onChange={(event) => setConfig({ updateUrl: event.target.value })} />
        </div>
        <div className="flex gap-2 py-4">
          <Button variant="outline" disabled={app.busy === "tools"} onClick={() => void checkUpdates()}>
            {t("check_updates")}
          </Button>
          <Button variant="ghost" disabled={app.busy === "tools"} onClick={() => void updateYtDlp()}>
            {t("update_ytdlp")}
          </Button>
          <Button variant="ghost" disabled={app.busy === "tools"} onClick={() => void reinstallTools()}>
            {t("reinstall_tools")}
          </Button>
          <Button variant="outline" disabled={app.busy === "tools"} onClick={() => void makeDiagnostic()}>
            {t("settings_diagnostic")}
          </Button>
        </div>
        <p className="pb-3 text-[13px] leading-relaxed text-muted">{t("settings_diagnostic_hint")}</p>
        <div className="pb-4">
          <Button variant="ghost" disabled={app.busy === "tools"} onClick={() => void measureLibrary()}>
            {t("settings_gain")}
          </Button>
          <p className="mt-2 text-[13px] leading-relaxed text-muted">{t("player_normalize_lufs")}</p>
        </div>
        {app.tools.message ? <p className="pb-3 text-[13px] text-danger">{app.tools.message}</p> : null}
      </Section>
      <LibrarySection />
      <DriveSection />
      <Section title={t("settings_gemini")}>
        <DriveExtras />
      </Section>
      <Section title={t("settings_discord")}>
        <p className="py-3 text-[13px] leading-relaxed text-muted">{t("settings_discord_hint")}</p>
        <Row label={t("settings_discord")}>
          <Switch label={t("settings_discord")} checked={config.discord} onCheckedChange={(value) => setConfig({ discord: value })} />
        </Row>
        <Input className="mt-3" value={config.discordClientId} placeholder={t("settings_discord_id")} onChange={(event) => setConfig({ discordClientId: event.target.value })} />
      </Section>
      <Section title={t("about")}>
        <p className="py-3 text-[14px]">bob baixa · {t("version")}</p>
        <div className="pb-4">
          <Button variant="outline" onClick={openNotes}>
            {t("whats_new")}
          </Button>
        </div>
        <p className="max-w-[52ch] pb-2 text-[13px] leading-relaxed text-muted">{t("extension_hint")}</p>
        <p className="max-w-[52ch] pb-4 text-[13px] leading-relaxed text-muted">{t("protocol")}</p>
      </Section>
    </div>
  )
}

function LibrarySection() {
  const app = useApp()
  const t = useT()
  const config = app.config
  return (
    <>
      <Section title={t("settings_acoustid")}>
        <p className="py-3 text-[13px] leading-relaxed text-muted">{t("settings_acoustid_hint")}</p>
        <Input value={config.acoustidKey} onChange={(event) => setConfig({ acoustidKey: event.target.value })} placeholder="acoustid" />
        <div className="mt-4">
          <Row label={t("settings_grab_lyrics")} hint={t("settings_grab_lyrics_hint")}>
            <Switch label={t("settings_grab_lyrics")} checked={config.grabLyrics} onCheckedChange={(value) => setConfig({ grabLyrics: value })} />
          </Row>
          <div className="border-b border-line py-4">
            <div className="text-[14px]">{t("settings_translate")}</div>
            <Select className="mt-3 w-[160px]" value={config.translateLang} onChange={(event) => setConfig({ translateLang: event.target.value })}>
              <option value="pt">português</option>
              <option value="en">english</option>
              <option value="es">español</option>
              <option value="fr">français</option>
            </Select>
          </div>
        </div>
      </Section>
      <Section title={t("settings_hotkeys")}>
        <p className="py-3 text-[13px] leading-relaxed text-muted">{t("settings_hotkeys_hint")}</p>
        <Hotkey label={t("settings_hotkey_play")} value={config.hotkeyPlay} onChange={(hotkeyPlay) => setConfig({ hotkeyPlay })} />
        <Hotkey label={t("settings_hotkey_next")} value={config.hotkeyNext} onChange={(hotkeyNext) => setConfig({ hotkeyNext })} />
        <Hotkey label={t("settings_hotkey_prev")} value={config.hotkeyPrev} onChange={(hotkeyPrev) => setConfig({ hotkeyPrev })} />
        <Hotkey label={t("settings_hotkey_like")} value={config.hotkeyLike} onChange={(hotkeyLike) => setConfig({ hotkeyLike })} />
      </Section>
    </>
  )
}

function Hotkey({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-line py-3">
      <span className="text-[14px]">{label}</span>
      <Input className="w-[180px]" value={value} onChange={(event) => onChange(event.target.value)} />
    </div>
  )
}

interface DriveStatus {
  enabled: boolean
  connected: boolean
  pending: number
  email?: string
}

interface DriveItem {
  path: string
  name: string
  phase: string
  percent: number
  error: string
  link: string
}

function formatBytes(value: number) {
  if (!value) return "0 MB"
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)} GB`
  return `${Math.max(1, Math.round(value / 1_000_000))} MB`
}

function DriveSection() {
  const app = useApp()
  const t = useT()
  const [status, setStatus] = useState<DriveStatus | null>(null)
  const [items, setItems] = useState<DriveItem[]>([])
  const [note, setNote] = useState("")
  const [busy, setBusy] = useState(false)
  const refresh = () => {
    if (!isTauri()) return
    void invoke<DriveStatus>("drive_status").then(setStatus).catch(() => setStatus(null))
    void invoke<DriveItem[]>("drive_queue").then(setItems).catch(() => setItems([]))
  }
  useEffect(() => {
    refresh()
    if (!isTauri()) return
    const timer = window.setInterval(refresh, 1500)
    return () => window.clearInterval(timer)
  }, [])
  const built = typeof __BOB_DRIVE__ !== "undefined" && __BOB_DRIVE__
  if (!built && !status?.enabled) return null
  return (
    <Section title={t("settings_drive")}>
      <p className="py-3 text-[13px] leading-relaxed text-muted">{t("settings_drive_hint")}</p>
      <p className="pb-3 text-[13px] text-muted">
        {status?.connected ? `${t("settings_drive_connected")}${status.email ? ` · ${status.email}` : ""}` : ""}
        {status ? ` · ${t("settings_drive_pending").replace("{n}", String(status.pending))}` : ""}
      </p>
      <div className="flex flex-wrap gap-2 pb-3">
        <Button
          disabled={busy}
          onClick={() => {
            setBusy(true)
            setNote("")
            void invoke<DriveStatus>("drive_connect")
              .then((next) => {
                setStatus(next)
                setNote(next.email ? `${t("settings_drive_connected")} · ${next.email}` : t("settings_drive_connected"))
                refresh()
              })
              .catch((error: unknown) => setNote(error instanceof Error ? error.message : String(error)))
              .finally(() => setBusy(false))
          }}
        >
          {t("settings_drive_in")}
        </Button>
        <Button
          variant="outline"
          onClick={() => {
            void invoke("drive_disconnect").then(() => setStatus((current) => (current ? { ...current, connected: false } : current)))
          }}
        >
          {t("settings_drive_out")}
        </Button>
        <Button
          variant="ghost"
          disabled={!status?.connected}
          onClick={() => {
            setBusy(true)
            void invoke<number>("drive_upload_library")
              .then((count) => setNote(t("settings_drive_pending").replace("{n}", String(count))))
              .catch((error: unknown) => setNote(error instanceof Error ? error.message : String(error)))
              .finally(() => setBusy(false))
          }}
        >
          {t("settings_drive_library")}
        </Button>
        <Button
          variant="outline"
          disabled={!status?.connected || busy}
          onClick={() => {
            setBusy(true)
            setNote("")
            void invoke<{ email: string; usage: number; limit: number }>("drive_test")
              .then((account) => {
                const quota = t("settings_drive_quota")
                  .replace("{used}", formatBytes(account.usage))
                  .replace("{limit}", account.limit ? formatBytes(account.limit) : "—")
                setNote(`${account.email} · ${quota}`)
                refresh()
              })
              .catch((error: unknown) => setNote(error instanceof Error ? error.message : String(error)))
              .finally(() => setBusy(false))
          }}
        >
          {t("settings_drive_test")}
        </Button>
        <Button variant="ghost" onClick={() => void invoke("drive_open", { target: "log" })}>
          {t("settings_drive_log")}
        </Button>
        <Button
          variant="outline"
          disabled={!status?.connected || busy}
          onClick={() => {
            setBusy(true)
            setNote(t("settings_drive_repairing"))
            void invoke<{ tracks: number; lyrics: number; covers: number }>("drive_repair")
              .then((report) => {
                setNote(
                  t("settings_drive_repaired")
                    .replace("{tracks}", String(report.tracks))
                    .replace("{lyrics}", String(report.lyrics))
                    .replace("{covers}", String(report.covers)),
                )
                refresh()
              })
              .catch((error: unknown) => setNote(error instanceof Error ? error.message : String(error)))
              .finally(() => setBusy(false))
          }}
        >
          {t("settings_drive_repair")}
        </Button>
      </div>
      <ul className="pb-2">
        {items.map((item) => (
          <li key={item.path} className="flex items-center justify-between gap-3 border-b border-line py-2 text-[13px]">
            <span className="min-w-0">
              <span className="block truncate">{item.name}</span>
              <span className="text-muted">
                {item.phase === "uploading"
                  ? t("settings_drive_sending").replace("{n}", String(item.percent))
                  : item.phase === "sent"
                    ? t("settings_drive_sent")
                    : item.phase === "failed"
                      ? item.error || t("player_failed")
                      : t("settings_drive_waiting")}
              </span>
            </span>
            {item.phase === "sent" && item.link ? (
              <button type="button" className="shrink-0 text-muted hover:text-ink" onClick={() => void invoke("drive_open", { target: item.link })}>
                {t("settings_drive_open")}
              </button>
            ) : null}
            {item.phase === "failed" ? (
              <button type="button" className="shrink-0 text-muted hover:text-ink" onClick={() => void invoke("drive_retry", { path: item.path }).then(refresh)}>
                {t("settings_drive_retry")}
              </button>
            ) : null}
          </li>
        ))}
      </ul>
      <Row label={t("settings_drive_only")} hint={t("settings_drive_only_hint")}>
        <Switch label={t("settings_drive_only")} checked={app.config.driveOnly} onCheckedChange={(value) => setConfig({ driveOnly: value })} />
      </Row>
      {note ? <p className="py-2 text-[13px] text-muted">{note}</p> : null}
    </Section>
  )
}

function DriveExtras() {
  const t = useT()
  const app = useApp()
  const [key, setKey] = useState("")
  const [gemini, setGemini] = useState(false)
  const [trash, setTrash] = useState<{ id: string; name: string }[]>([])
  const [openTrash, setOpenTrash] = useState(false)
  useEffect(() => {
    if (!isTauri()) return
    void invoke<boolean>("gemini_status").then(setGemini).catch(() => setGemini(false))
  }, [])
  return (
    <div className="space-y-3 border-t border-line py-3">
      <Row label={t("settings_queue_new")} hint={t("settings_queue_new_hint")}>
        <Switch label={t("settings_queue_new")} checked={app.config.queueNewDownloads} onCheckedChange={(value) => setConfig({ queueNewDownloads: value })} />
      </Row>
      <Row label={t("settings_queue_where")}>
        <Select value={app.config.queueNewWhere} onChange={(event) => setConfig({ queueNewWhere: event.target.value === "next" ? "next" : "end" })}>
          <option value="end">{t("settings_queue_end")}</option>
          <option value="next">{t("settings_queue_next")}</option>
        </Select>
      </Row>
      <Row label={t("settings_discover_ratio")}>
        <input
          type="range"
          min={0}
          max={80}
          value={Math.round(app.config.discoverRatio * 100)}
          onChange={(event) => setConfig({ discoverRatio: Number(event.target.value) / 100 })}
        />
      </Row>
      <div>
        <div className="text-[14px]">{t("settings_gemini")}</div>
        <p className="mt-1 text-[12px] text-muted">{t("settings_gemini_hint")}</p>
        <p className="mt-1 text-[12px] text-muted">{gemini ? t("settings_gemini_on") : t("settings_gemini_off")}</p>
        <div className="mt-2 flex gap-2">
          <Input type="password" value={key} onChange={(event) => setKey(event.target.value)} autoComplete="off" />
          <Button
            type="button"
            onClick={() => {
              void invoke("gemini_save_key", { key }).then(() => {
                setGemini(Boolean(key.trim()))
                setKey("")
              })
            }}
          >
            {t("settings_gemini_save")}
          </Button>
          <Button type="button" variant="ghost" onClick={() => void invoke("gemini_save_key", { key: "" }).then(() => setGemini(false))}>
            {t("settings_gemini_clear")}
          </Button>
        </div>
        <div className="mt-3 grid gap-3">
          <Input value={app.config.geminiFlash} onChange={(event) => setConfig({ geminiFlash: event.target.value })} />
          <Input value={app.config.geminiFlashFallback} onChange={(event) => setConfig({ geminiFlashFallback: event.target.value })} />
          <Input value={app.config.geminiBulk} onChange={(event) => setConfig({ geminiBulk: event.target.value })} />
          <Input value={app.config.geminiEmbed} onChange={(event) => setConfig({ geminiEmbed: event.target.value })} />
          <Input value={app.config.geminiTranscribe} onChange={(event) => setConfig({ geminiTranscribe: event.target.value })} />
        </div>
        <div className="py-3">
          <Row label={t("settings_gemini_auto")}>
            <Switch label={t("settings_gemini_auto")} checked={app.config.geminiAutoLyrics} onCheckedChange={(value) => setConfig({ geminiAutoLyrics: value })} />
          </Row>
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={() => {
            void invoke<{ usage: number; limit: number; offline: number; cloud: number; cacheBytes: number }>("drive_space").then((space) => {
              window.alert(
                t("settings_space_line")
                  .replace("{used}", formatBytes(space.usage))
                  .replace("{offline}", String(space.offline))
                  .replace("{cloud}", String(space.cloud))
                  .replace("{cache}", formatBytes(space.cacheBytes)),
              )
            })
          }}
        >
          {t("settings_space")}
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => {
            void invoke<number>("free_cache").then((freed) => window.alert(t("settings_freed").replace("{n}", formatBytes(freed))))
          }}
        >
          {t("settings_free_cache")}
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => {
            setOpenTrash(true)
            void invoke<{ id: string; name: string }[]>("list_trash").then(setTrash).catch(() => setTrash([]))
          }}
        >
          {t("settings_trash")}
        </Button>
      </div>
      {openTrash ? (
        <ul>
          {trash.length === 0 ? <li className="text-[13px] text-muted">{t("settings_trash_empty")}</li> : null}
          {trash.map((item) => (
            <li key={item.id} className="flex items-center justify-between gap-3 border-b border-line py-2 text-[13px]">
              <span className="truncate">{item.name}</span>
              <span className="flex gap-3">
                <button type="button" className="text-muted hover:text-ink" onClick={() => void invoke("restore_trash", { id: item.id }).then(() => setTrash((rows) => rows.filter((row) => row.id !== item.id)))}>
                  {t("settings_restore")}
                </button>
                <button type="button" className="text-muted hover:text-ink" onClick={() => void invoke("purge_trash", { id: item.id }).then(() => setTrash((rows) => rows.filter((row) => row.id !== item.id)))}>
                  {t("settings_purge")}
                </button>
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}

function ThemePicker({
  theme,
  onPick,
  systemLabel,
}: {
  theme: ThemeMode
  onPick: (theme: ThemeMode) => void
  systemLabel: string
}) {
  const t = useT()
  const choices: { id: ThemeMode; label: string; swatch: string }[] = [
    { id: "light", label: t("theme_light"), swatch: "border-[#e4e4e1] bg-[#f7f7f5]" },
    { id: "dark", label: t("theme_dark"), swatch: "border-[#2c2c2c] bg-[#161616]" },
    {
      id: "glass-light",
      label: t("theme_glass_light"),
      swatch: "border-white/80 bg-white/45 shadow-[0_6px_16px_rgba(40,36,30,0.08)] backdrop-blur-md",
    },
    {
      id: "glass-dark",
      label: t("theme_glass_dark"),
      swatch: "border-white/20 bg-white/10 shadow-[0_6px_16px_rgba(0,0,0,0.35)] backdrop-blur-md",
    },
  ]
  return (
    <div className="flex flex-col items-end gap-3">
      <button
        type="button"
        data-theme="system"
        onClick={() => onPick("system")}
        className={cn(
          "h-8 rounded-full px-3 text-[13px]",
          theme === "system" ? "bg-fill text-onfill" : "text-muted hover:text-ink",
        )}
      >
        {systemLabel}
      </button>
      <div className="flex gap-2">
        {choices.map((choice) => (
          <button
            key={choice.id}
            type="button"
            data-theme={choice.id}
            onClick={() => onPick(choice.id)}
            className={cn("w-[78px] rounded-xl p-1 text-left", theme === choice.id ? "ring-1 ring-ink" : "")}
          >
            <span className={cn("relative block h-11 overflow-hidden rounded-lg border", choice.swatch)}>
              {choice.id === "glass-light" ? <span className="absolute inset-0 bg-[radial-gradient(circle_at_20%_20%,rgba(255,255,255,.9),transparent_55%),linear-gradient(160deg,#e7e2da,#cfc9c0)]" /> : null}
              {choice.id === "glass-dark" ? <span className="absolute inset-0 bg-[radial-gradient(circle_at_20%_20%,rgba(255,255,255,.28),transparent_55%),linear-gradient(160deg,#2c2c32,#121214)]" /> : null}
            </span>
            <span className="mt-1 block text-center text-[11px] leading-tight text-muted">{choice.label}</span>
          </button>
        ))}
      </div>
    </div>
  )
}

function Section({ title, first, children }: { title: string; first?: boolean; children: ReactNode }) {
  return (
    <section className={first ? "" : "mt-10"}>
      <h2 className="mb-1 text-[13px] text-muted">{title}</h2>
      {children}
    </section>
  )
}

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-8 border-b border-line py-4">
      <div className="max-w-[300px]">
        <div className="text-[14px]">{label}</div>
        {hint ? <p className="mt-1 text-[13px] leading-relaxed text-muted">{hint}</p> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  )
}
