import { PageHeader } from "../components/page-header"
import { Progress } from "../components/progress"
import { Thumb } from "../components/thumb"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { formatBytes, formatClock, formatSpeed, formatWhen } from "../lib/format"
import type { Job, JobState } from "../lib/types"
import type { Key } from "../lib/i18n"
import { clearSchedule, jobAction, openOutput, scheduleQueue, setField, useApp, useT } from "../store"

const STATE_KEY: Record<JobState, Key> = {
  queued: "state_queued",
  scheduled: "state_scheduled",
  downloading: "state_downloading",
  paused: "state_paused",
  done: "state_done",
  failed: "state_failed",
  cancelled: "state_cancelled",
}

export function Queue() {
  const app = useApp()
  const t = useT()
  return (
    <div>
      <PageHeader title={t("queue_title")} sub={t("queue_sub")} />
      <div className="mb-8 flex flex-wrap items-end gap-3">
        <label className="block">
          <span className="mb-1.5 block text-[12px] text-muted">{t("schedule")}</span>
          <Input type="time" className="w-[140px]" value={app.scheduleClock} onChange={(event) => setField("scheduleClock", event.target.value)} />
        </label>
        <Button variant="outline" onClick={() => void scheduleQueue()}>
          {t("schedule_btn")}
        </Button>
        {app.scheduleAt ? (
          <Button variant="ghost" onClick={() => void clearSchedule()}>
            {t("schedule_clear")}
          </Button>
        ) : null}
      </div>
      {app.scheduleAt ? (
        <p className="mb-6 text-[13px] text-muted">{t("schedule_on", { time: formatWhen(app.scheduleAt, app.config.language) })}</p>
      ) : (
        <p className="mb-6 text-[13px] leading-relaxed text-muted">{t("schedule_hint")}</p>
      )}
      {app.jobs.length === 0 ? (
        <div className="border-t border-line py-16">
          <p className="text-[15px]">{t("empty_queue")}</p>
          <p className="mt-2 text-[13px] text-muted">{t("empty_queue_hint")}</p>
        </div>
      ) : (
        <ul className="border-t border-line">
          {app.jobs.map((job) => (
            <JobRow key={job.id} job={job} />
          ))}
        </ul>
      )}
    </div>
  )
}

function JobRow({ job }: { job: Job }) {
  const app = useApp()
  const t = useT()
  const meta = [
    t(STATE_KEY[job.state]),
    job.audioOnly ? job.audioFormat ?? "mp3" : `${job.quality === "best" ? t("best") : job.quality === "2160" ? "4k" : `${job.quality}p`} · ${job.container}`,
    job.sectionStart && job.sectionEnd ? `${t("recorte")} ${job.sectionStart}–${job.sectionEnd}` : null,
    job.speedLimit ? `${t("limit")} ${job.speedLimit}` : null,
  ]
    .filter(Boolean)
    .join(" · ")
  return (
    <li className="border-b border-line py-5">
      <div className="flex gap-4">
        <Thumb src={job.thumbnail} videoUrl={job.url} className="h-[72px] w-[128px]" />
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-4">
            <h2 className="truncate text-[15px] font-medium tracking-tight select-text">{job.title}</h2>
            <span className="shrink-0 text-[12px] tabular-nums text-muted">
              {job.state === "downloading" ? `${Math.round(job.progress * 100)}%` : t(STATE_KEY[job.state])}
            </span>
          </div>
          <p className="mt-1 truncate text-[13px] text-muted">{meta}</p>
          <div className="mt-3">
            <Progress value={job.progress} active={job.state === "downloading"} />
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[12px] tabular-nums text-muted">
            {job.state === "downloading" ? (
              <>
                <span>{formatSpeed(job.speed, app.config.language)}</span>
                <span>
                  {t("eta")} {formatClock(job.eta)}
                </span>
                {job.total ? (
                  <span>
                    {formatBytes(job.downloaded ?? 0, app.config.language)} / {formatBytes(job.total, app.config.language)}
                  </span>
                ) : null}
              </>
            ) : null}
            {job.scheduledAt && job.state === "scheduled" ? <span>{t("scheduled_for", { time: formatWhen(job.scheduledAt, app.config.language) })}</span> : null}
          </div>
          {job.error ? <p className="mt-2 text-[13px] text-danger">{job.error}</p> : null}
          <div className="mt-3 flex flex-wrap gap-1">
            {job.state === "downloading" || job.state === "queued" ? (
              <Button size="sm" variant="ghost" onClick={() => void jobAction(job.id, "pause")}>
                {t("pause")}
              </Button>
            ) : null}
            {job.state === "paused" || job.state === "scheduled" ? (
              <Button size="sm" variant="ghost" onClick={() => void jobAction(job.id, "resume")}>
                {t("resume")}
              </Button>
            ) : null}
            {job.state === "failed" || job.state === "cancelled" ? (
              <Button size="sm" variant="ghost" onClick={() => void jobAction(job.id, "retry")}>
                {t("retry")}
              </Button>
            ) : null}
            {job.state === "downloading" || job.state === "queued" || job.state === "paused" || job.state === "scheduled" ? (
              <Button size="sm" variant="quiet" onClick={() => void jobAction(job.id, "cancel")}>
                {t("cancel")}
              </Button>
            ) : null}
            {job.outputPath ? (
              <Button size="sm" variant="quiet" onClick={() => void openOutput(job.outputPath!)}>
                {t("open_folder")}
              </Button>
            ) : null}
            <Button size="sm" variant="quiet" onClick={() => void jobAction(job.id, "remove")}>
              {t("remove")}
            </Button>
          </div>
        </div>
      </div>
    </li>
  )
}
