import { useState } from "react"
import { cancelAi, useAiJobs } from "../lib/ai-activity"
import { useT } from "../store"

function clock(ms: number) {
  const seconds = Math.max(0, ms) / 1000
  return seconds < 10 ? `${seconds.toFixed(1)}s` : `${Math.round(seconds)}s`
}

export function AiActivity() {
  const jobs = useAiJobs()
  const t = useT()
  const [open, setOpen] = useState(false)
  const running = jobs.filter((job) => job.phase === "running")
  const current = running[0]
  return (
    <>
      <button
        type="button"
        className="mt-3 flex w-full items-center gap-2 rounded-xl px-1 py-1 text-left text-[12px] text-muted hover:text-ink"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
      >
        <span className={running.length ? "h-1.5 w-1.5 animate-pulse rounded-full bg-ink" : "h-1.5 w-1.5 rounded-full bg-line"} />
        <span className="min-w-0 flex-1 truncate">{current ? current.label : t("ai_idle")}</span>
      </button>
      {open ? (
        <div className="fixed bottom-4 left-4 z-40 w-[320px] rounded-2xl border border-line bg-surface p-3 text-ink shadow-lg" role="dialog" aria-label={t("ai_panel")}>
          <div className="mb-2 flex items-center justify-between">
            <p className="text-[13px] font-medium">{t("ai_panel")}</p>
            <button type="button" className="text-[12px] text-muted" onClick={() => setOpen(false)}>
              {t("close")}
            </button>
          </div>
          {jobs.length === 0 ? <p className="text-[12px] text-muted">{t("ai_idle")}</p> : null}
          <ul className="max-h-72 space-y-2 overflow-auto">
            {jobs.map((job) => (
              <li key={job.id} className="rounded-xl bg-soft px-3 py-2">
                <p className="text-[13px]">{job.label}</p>
                <p className="mt-1 text-[11px] text-muted">
                  {job.phase === "running" ? t("ai_running") : job.phase === "done" ? t("ai_done") : job.phase === "cancelled" ? t("ai_cancel") : t("ai_error")}
                  {job.model ? ` · ${t("ai_model")} ${job.model}` : ""}
                  {job.ended ? ` · ${clock(job.ended - job.started)}` : ""}
                </p>
                {job.detail ? <p className="mt-1 text-[12px] leading-snug">{job.detail}</p> : null}
                {job.phase === "running" ? (
                  <button type="button" className="mt-2 text-[12px] text-muted hover:text-ink" onClick={() => cancelAi(job.id)}>
                    {t("ai_cancel")}
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </>
  )
}
