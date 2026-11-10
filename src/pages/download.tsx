import { useState } from "react"
import { Ellipsis } from "lucide-react"
import { Progress } from "../components/progress"
import { Segmented } from "../components/segmented"
import type { Job, JobState, Quality } from "../lib/types"
import type { Key } from "../lib/i18n"
import { enqueueItems, jobAction, setConfig, setDraft, useApp, useT } from "../store"

const QUALITIES: { value: Quality; key: Key }[] = [
  { value: "best", key: "best" },
  { value: "1080", key: "p1080" },
  { value: "720", key: "p720" },
  { value: "480", key: "p480" },
]

const STATE_KEY: Record<JobState, Key> = {
  queued: "state_queued",
  scheduled: "state_scheduled",
  downloading: "state_downloading",
  paused: "state_paused",
  done: "state_done",
  failed: "state_failed",
  cancelled: "state_cancelled",
}

export function Download() {
  const app = useApp()
  const t = useT()
  const [more, setMore] = useState(false)
  const format = app.config.audioOnly ? "mp3" : "mp4"

  function choose(next: "mp3" | "mp4") {
    if (next === "mp3") setConfig({ audioOnly: true, audioFormat: "mp3" })
    else setConfig({ audioOnly: false, container: "mp4" })
  }

  async function start() {
    const url = app.draft.trim()
    if (!url) return
    const audio = format === "mp3"
    await enqueueItems([
      {
        url,
        title: url,
        audioOnly: audio,
        container: audio ? undefined : "mp4",
        audioFormat: audio ? "mp3" : undefined,
      },
    ])
    setDraft("")
  }

  return (
    <div>
      <h1 className="text-[28px] font-medium tracking-[-0.04em]">{t("nav_download")}</h1>
      <form
        className="mt-6 flex items-center gap-3"
        onSubmit={(event) => {
          event.preventDefault()
          void start()
        }}
      >
        <input
          value={app.draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder={t("paste_found")}
          aria-label={t("paste_found")}
          className="h-14 min-w-0 flex-1 rounded-2xl border border-line bg-surface px-5 text-[16px] outline-none"
        />
        <button id="download-go" type="submit" className="h-14 rounded-full px-6 text-[15px] font-medium" style={{ background: "var(--accent)", color: "#101010" }}>
          {t("download_now")}
        </button>
      </form>
      <div className="mt-4 flex items-center gap-3">
        <Segmented
          value={format}
          onChange={choose}
          options={[
            { value: "mp3", label: "MP3" },
            { value: "mp4", label: "MP4" },
          ]}
        />
        <button type="button" aria-label={t("player_more")} title={t("player_more")} className="grid h-9 w-9 place-items-center rounded-full text-muted hover:bg-soft hover:text-ink" onClick={() => setMore((value) => !value)}>
          <Ellipsis className="h-4 w-4" />
        </button>
      </div>
      {more ? (
        <div className="mt-3 flex flex-wrap gap-2">
          {QUALITIES.map((item) => (
            <button
              key={item.value}
              type="button"
              className="h-8 rounded-full bg-soft px-3 text-[13px] text-muted hover:text-ink"
              aria-pressed={app.config.quality === item.value}
              onClick={() => setConfig({ quality: item.value })}
            >
              {t(item.key)}
            </button>
          ))}
        </div>
      ) : null}
      <section className="mt-10">
        {app.jobs.length === 0 ? (
          <div className="max-w-[420px] py-8">
            <p className="text-[16px] leading-relaxed">{t("empty_queue")}</p>
            <p className="mt-2 text-[14px] text-muted">{t("empty_queue_hint")}</p>
          </div>
        ) : (
          <ul className="space-y-3">
            {app.jobs.map((job) => (
              <JobLine key={job.id} job={job} />
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}

function JobLine({ job }: { job: Job }) {
  const t = useT()
  const live = job.state === "downloading" || job.state === "queued" || job.state === "paused"
  return (
    <li className="rounded-2xl bg-surface px-4 py-3">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[14px]">{job.title}</p>
          <p className="mt-1 text-[12px] text-muted">{job.state === "downloading" ? `${Math.round(job.progress * 100)}%` : t(STATE_KEY[job.state])}</p>
        </div>
        {live ? (
          <button type="button" className="text-[12px] text-muted hover:text-ink" onClick={() => void jobAction(job.id, "cancel")}>
            {t("cancel")}
          </button>
        ) : null}
        {job.state === "failed" || job.state === "cancelled" ? (
          <button type="button" className="text-[12px] text-muted hover:text-ink" onClick={() => void jobAction(job.id, "retry")}>
            {t("retry")}
          </button>
        ) : null}
      </div>
      <div className="mt-3">
        <Progress value={job.progress} active={job.state === "downloading" || job.state === "queued"} />
      </div>
    </li>
  )
}
