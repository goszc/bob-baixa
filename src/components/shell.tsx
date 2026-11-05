import type { ReactNode } from "react"
import { translate, type Key } from "../lib/i18n"
import type { Page, ToolPhase } from "../lib/types"
import { cn } from "../lib/cn"
import { applyUpdate, closeConfirm, dismissNotes, go, useApp, useT } from "../store"
import { ConfirmDialog, NotesDialog } from "./ui/dialog"
import { Logo } from "./logo"
import { NowPlaying } from "./now-playing"
import { PlayerEngine } from "./player-engine"

const NAV: { id: Page; key: Key }[] = [
  { id: "home", key: "nav_download" },
  { id: "player", key: "nav_player" },
  { id: "queue", key: "nav_queue" },
  { id: "search", key: "nav_search" },
  { id: "convert", key: "nav_convert" },
  { id: "monitor", key: "nav_monitor" },
  { id: "history", key: "nav_history" },
  { id: "settings", key: "nav_settings" },
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
      {count ? <span className="ml-auto text-[12px] tabular-nums text-muted">{count}</span> : null}
    </button>
  )
}

export function Shell({ children }: { children: ReactNode }) {
  const app = useApp()
  const t = useT()
  const queued = app.jobs.filter((job) => job.state === "queued" || job.state === "downloading" || job.state === "scheduled" || job.state === "paused").length
  return (
    <div className="flex h-full min-h-screen bg-bg text-ink">
      <aside className="hidden w-[232px] shrink-0 flex-col border-r border-line px-4 py-6 md:flex">
        <div className="mb-8 flex items-center gap-2.5 px-3">
          <Logo className="h-[18px] w-[26px]" />
          <span className="text-[15px] font-medium tracking-[-0.03em]">bob baixa</span>
        </div>
        <nav className="flex flex-col gap-1">
          {NAV.map((item) => (
            <NavButton key={item.id} id={item.id} label={t(item.key)} count={item.id === "queue" ? queued : undefined} />
          ))}
        </nav>
        <div className="mt-auto px-3 pt-6">
          <ToolLine name="yt-dlp" phase={app.tools.ytdlp} />
          <ToolLine name="ffmpeg" phase={app.tools.ffmpeg} />
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
        <main className={cn("min-h-0 flex-1", app.page === "player" ? "overflow-hidden" : "overflow-auto")}>
          {app.page === "player" ? (
            <div key="player" className="animate-rise h-full">
              {children}
            </div>
          ) : (
            <div key={app.page} className="animate-rise max-w-[760px] px-8 py-10 md:px-12">
              {children}
            </div>
          )}
        </main>
        {app.page === "player" ? null : <NowPlaying />}
      </div>
      <PlayerEngine />
      {app.updateOffer ? (
        <div className={cn("fixed left-1/2 z-30 flex -translate-x-1/2 items-center gap-3 rounded-full bg-fill px-4 py-2 text-[13px] text-onfill", app.page === "player" ? "bottom-6" : "bottom-24")}>
          <span>{t("update_ready")}</span>
          <button type="button" className="underline" onClick={() => void applyUpdate()}>
            {t("restart")}
          </button>
        </div>
      ) : app.toast ? (
        <div className={cn("pointer-events-none fixed left-1/2 z-30 -translate-x-1/2 rounded-full bg-fill px-4 py-2 text-[13px] text-onfill", app.page === "player" ? "bottom-6" : "bottom-24")}>
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
    </div>
  )
}
