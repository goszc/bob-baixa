import { useEffect, useState, type ReactNode } from "react"
import { translate, type Key } from "../lib/i18n"
import type { Page, ToolPhase } from "../lib/types"
import { cn } from "../lib/cn"
import { useMusic } from "../lib/music"
import { playTracks, restoreSession } from "../lib/player"
import { useLibraryTracks } from "../lib/use-library"
import { applyUpdate, closeConfirm, dismissNotes, go, runSearch, setDraft, setSearchQuery, useApp, useT } from "../store"
import { ConfirmDialog, NotesDialog } from "./ui/dialog"
import { Logo } from "./logo"
import { NowPlaying } from "./now-playing"
import { PlayerEngine } from "./player-engine"
import { AiActivity } from "./ai-activity"
import { FullscreenLyrics } from "./fullscreen-lyrics"
import { SidePanel } from "./side-panel"
import { Thumb } from "./thumb"
import { closeMenus } from "../lib/menus"
import { seedDemoMusic } from "../lib/demo-library"
import { isTauri } from "../lib/api"
import { useCatalogSettled } from "../lib/catalog"
import { pressLyrics, setChrome, useChrome } from "../lib/chrome"

const NAV: { id: Page; key: Key }[] = [
  { id: "home", key: "nav_home" },
  { id: "likes", key: "nav_likes" },
  { id: "search", key: "nav_search" },
  { id: "library", key: "nav_library" },
  { id: "discover", key: "nav_discover" },
  { id: "download", key: "nav_download" },
  { id: "playlists", key: "nav_playlists" },
]

function ToolLine({ name, phase }: { name: string; phase: ToolPhase }) {
  const t = useT()
  const label =
    phase === "ready" ? t("tools_ready") : phase === "downloading" ? t("tools_downloading") : phase === "error" ? t("tools_error") : t("tools_missing")
  return (
    <div className="flex items-center gap-2 py-1 text-[12px] text-muted">
      <span
        className={cn(
          "h-1.5 w-1.5 rounded-full",
          phase === "ready" && "bg-ink",
          phase === "downloading" && "animate-pulse bg-ink/50",
          (phase === "missing" || phase === "error") && "bg-line",
        )}
      />
      <span>{name}</span>
      <span className="ml-auto">{label}</span>
    </div>
  )
}

function NavButton({ id, label, count }: { id: Page; label: string; count?: number }) {
  const page = useApp().page
  const active = page === id
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={() => go(id)}
      className={cn(
        "flex h-10 w-full items-center rounded-[10px] px-3 text-left text-[14px] transition-colors",
        active ? "bg-soft text-ink" : "text-muted hover:bg-soft/80 hover:text-ink",
      )}
    >
      <span>{label}</span>
      {count != null ? <span className="ml-auto text-[12px] tabular-nums text-muted">{count}</span> : null}
    </button>
  )
}

function QueueRing({ progress, label }: { progress: number; label: string }) {
  const radius = 8
  const circ = 2 * Math.PI * radius
  const offset = circ * (1 - Math.min(1, Math.max(0, progress)))
  return (
    <button type="button" aria-label={label} title={label} onClick={() => go("download")} className="grid h-9 w-9 place-items-center text-muted hover:text-ink">
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r={radius} fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="2" />
        <circle cx="10" cy="10" r={radius} fill="none" stroke="currentColor" strokeWidth="2" strokeDasharray={circ} strokeDashoffset={offset} strokeLinecap="round" transform="rotate(-90 10 10)" />
      </svg>
    </button>
  )
}

export function Shell({ children }: { children: ReactNode }) {
  seedDemoMusic()
  const app = useApp()
  const music = useMusic()
  const tracks = useLibraryTracks()
  const settled = useCatalogSettled()
  const chrome = useChrome()
  const t = useT()
  const [palette, setPalette] = useState(false)
  const [paletteQuery, setPaletteQuery] = useState("")
  const active = app.jobs.filter((job) => job.state === "queued" || job.state === "downloading" || job.state === "scheduled" || job.state === "paused")
  const progress = active.length ? active.reduce((sum, job) => sum + (job.progress || (job.state === "downloading" ? 0.35 : 0.08)), 0) / active.length : 0
  const ringLabel = active.length ? `${Math.round(progress * 100)}% · ${active.length}` : t("nav_download")
  const hits = tracks
    .filter((track) => `${track.title} ${track.artist}`.toLocaleLowerCase().includes(paletteQuery.trim().toLocaleLowerCase()))
    .slice(0, 8)
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const field = event.target instanceof HTMLElement && (event.target.tagName === "INPUT" || event.target.tagName === "TEXTAREA" || event.target.isContentEditable)
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault()
        setPalette(true)
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "v" && !field) {
        event.preventDefault()
        void navigator.clipboard
          .readText()
          .then((text) => {
            const url = text.trim()
            if (!/youtu\.be|youtube\.com/i.test(url)) return
            setDraft(url)
            go("download")
          })
          .catch(() => {})
      }
    }
    const onPointer = (event: PointerEvent) => {
      const node = event.target instanceof Element ? event.target : null
      if (node?.closest("[data-menu]")) return
      closeMenus()
    }
    window.addEventListener("keydown", onKey)
    window.addEventListener("pointerdown", onPointer)
    return () => {
      window.removeEventListener("keydown", onKey)
      window.removeEventListener("pointerdown", onPointer)
    }
  }, [])
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const field = event.target instanceof HTMLElement && (event.target.tagName === "INPUT" || event.target.tagName === "TEXTAREA" || event.target.isContentEditable)
      if (event.key === "Escape") {
        closeMenus()
        if (chrome.lyrics) setChrome({ lyrics: false, karaoke: false })
      }
      if (!field && !event.metaKey && !event.ctrlKey && !event.altKey && event.key.toLowerCase() === "l") {
        event.preventDefault()
        pressLyrics()
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [chrome.lyrics])
  useEffect(() => {
    restoreSession(tracks, settled || !isTauri())
  }, [tracks, settled])
  useEffect(() => {
    if (app.confirm || app.showNotes) closeMenus()
  }, [app.confirm, app.showNotes])
  return (
    <div className="flex h-full min-h-screen bg-bg text-ink">
      <aside className="hidden w-[232px] shrink-0 flex-col border-r border-line px-4 py-6 md:flex">
        <div className="mb-8 flex items-center gap-2.5 px-3">
          <Logo className="h-[18px] w-[26px]" />
          <span className="text-[15px] font-medium tracking-[-0.03em]">bob baixa</span>
        </div>
        <nav className="flex flex-col gap-1">
          {NAV.map((item) => (
            <NavButton key={item.id} id={item.id} label={t(item.key)} count={item.id === "likes" ? music.likes.length : undefined} />
          ))}
        </nav>
        <div className="mt-auto px-3 pt-6">
          <div className="mb-2 flex items-center">
            {active.length ? <QueueRing progress={progress} label={ringLabel} /> : null}
            <AiActivity />
          </div>
          <ToolLine name="yt-dlp" phase={app.tools.ytdlp} />
          <ToolLine name="ffmpeg" phase={app.tools.ffmpeg} />
          <button type="button" className="mt-3 text-[13px] text-muted hover:text-ink" onClick={() => go("settings")}>
            {t("nav_settings")}
          </button>
        </div>
      </aside>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <nav className="flex gap-1 overflow-x-auto border-b border-line px-4 py-3 md:hidden">
          {NAV.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => go(item.id)}
              className={cn(
                "h-9 shrink-0 rounded-full px-3 text-[13px]",
                app.page === item.id ? "bg-fill text-onfill" : "text-muted",
              )}
            >
              {t(item.key)}
            </button>
          ))}
        </nav>
        <div className="flex min-h-0 flex-1">
          <main className={cn("min-h-0 min-w-0 flex-1", app.page === "player" || app.page === "library" ? "overflow-hidden" : "overflow-auto")}>
            {app.page === "player" || app.page === "library" ? (
              <div key={app.page} className="animate-rise h-full">
                {children}
              </div>
            ) : app.page === "home" || app.page === "discover" || app.page === "download" || app.page === "likes" || app.page === "playlists" || app.page === "settings" || app.page === "queue" || app.page === "convert" || app.page === "monitor" || app.page === "history" || app.page === "search" ? (
              <div key={app.page} className="animate-rise px-6 py-8 md:px-8">
                {children}
              </div>
            ) : (
              <div key={app.page} className="animate-rise max-w-[760px] px-8 py-10 md:px-12">
                {children}
              </div>
            )}
          </main>
          {chrome.lyrics ? null : <SidePanel />}
        </div>
        {app.page === "player" || chrome.lyrics ? null : <NowPlaying />}
      </div>
      <PlayerEngine />
      {app.updateOffer ? (
        <div className="fixed bottom-[108px] left-1/2 z-30 flex -translate-x-1/2 items-center gap-3 rounded-full bg-fill px-4 py-2 text-[13px] text-onfill">
          <span>{t("update_ready", { version: app.updateVersion })}</span>
          <button type="button" className="underline" onClick={() => void applyUpdate()}>
            {t("update_install")}
          </button>
        </div>
      ) : app.toast ? (
        <div className="pointer-events-none fixed bottom-[108px] left-1/2 z-30 -translate-x-1/2 rounded-full bg-fill px-4 py-2 text-[13px] text-onfill">
          {app.toast}
        </div>
      ) : null}
      <NotesDialog
        open={app.showNotes}
        title={t("whats_new")}
        body={app.releaseNotes.trim() || t("notes_fallback")}
        close={t("whats_new_close")}
        onClose={() => void dismissNotes()}
      />
      <ConfirmDialog
        open={app.confirm !== null}
        title={app.confirm?.title ?? ""}
        body={app.confirm?.body ?? ""}
        confirm={app.confirm?.confirm ?? translate(app.config.language, "confirm")}
        cancel={app.confirm?.cancel ?? translate(app.config.language, "cancel")}
        onClose={closeConfirm}
        onConfirm={() => {
          const run = app.confirm?.run
          closeConfirm()
          run?.()
        }}
      />
      {palette ? (
        <div className="fixed inset-0 z-40 bg-black/45" onClick={() => setPalette(false)}>
          <div className="mx-auto mt-24 w-[min(560px,92vw)] rounded-2xl bg-surface p-3 shadow-lg" onClick={(event) => event.stopPropagation()}>
            <input
              autoFocus
              value={paletteQuery}
              onChange={(event) => setPaletteQuery(event.target.value)}
              placeholder={t("search_palette")}
              aria-label={t("search_palette")}
              className="h-11 w-full rounded-xl bg-soft px-3 text-[14px] outline-none"
            />
            <ul className="mt-2">
              {hits.map((track) => (
                <li key={track.path}>
                  <button
                    type="button"
                    className="flex h-12 w-full items-center gap-3 rounded-xl px-3 text-left hover:bg-soft"
                    onClick={() => {
                      playTracks([track], 0)
                      setPalette(false)
                    }}
                  >
                    <Thumb src={track.thumbnail} videoUrl={track.pageUrl || track.title} className="h-8 w-8 rounded-md" />
                    <span className="truncate text-[14px]">{track.title}</span>
                    <span className="ml-3 truncate text-[12px] text-muted">{track.artist}</span>
                  </button>
                </li>
              ))}
              <li>
                <button
                  type="button"
                  className="flex h-12 w-full items-center rounded-xl px-3 text-left text-[14px] hover:bg-soft"
                  onClick={() => {
                    setSearchQuery(paletteQuery)
                    go("search")
                    void runSearch()
                    setPalette(false)
                  }}
                >
                  {t("search_youtube")}
                </button>
              </li>
            </ul>
          </div>
        </div>
      ) : null}
      <FullscreenLyrics />
    </div>
  )
}
