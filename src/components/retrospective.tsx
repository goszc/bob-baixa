import { useMemo, useState } from "react"
import { invoke, isTauri } from "../lib/api"
import type { RetroRange } from "../lib/listening"
import { retrospective } from "../lib/listening"
import { useMusic } from "../lib/music"
import { useT } from "../store"

const RANGES: { value: RetroRange; key: "retro_week" | "retro_month" | "retro_year" | "retro_rolling" }[] = [
  { value: "week", key: "retro_week" },
  { value: "month", key: "retro_month" },
  { value: "year", key: "retro_year" },
  { value: "rolling", key: "retro_rolling" },
]

function minutesLabel(value: number) {
  if (value < 10) return value.toLocaleString(undefined, { maximumFractionDigits: 1 })
  return Math.round(value).toLocaleString()
}

export function Retrospective() {
  const music = useMusic()
  const t = useT()
  const [range, setRange] = useState<RetroRange>("week")
  const [wrapped, setWrapped] = useState("")
  const summary = useMemo(() => retrospective(music.events, range, Date.now()), [music.events, range])
  const top = Math.max(summary.tracks[0]?.ms ?? 0, summary.artists[0]?.ms ?? 0, 1)
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="mb-4 flex flex-wrap gap-2">
        {RANGES.map((item) => (
          <button
            key={item.value}
            type="button"
            aria-pressed={range === item.value}
            onClick={() => setRange(item.value)}
            className={range === item.value ? "h-8 rounded-full bg-fill px-3 text-[13px] text-onfill" : "h-8 rounded-full bg-soft px-3 text-[13px] text-muted"}
          >
            {t(item.key)}
          </button>
        ))}
      </div>
      <div className="grid grid-cols-3 gap-2">
        <Stat label={t("retro_minutes")} value={minutesLabel(summary.minutes)} />
        <Stat label={t("retro_streak")} value={t("retro_streak_days", { n: summary.currentStreak })} />
        <Stat label={t("retro_best")} value={t("retro_streak_days", { n: summary.periodStreak })} />
      </div>
      <button
        type="button"
        className="mt-3 text-left text-[13px] text-muted hover:text-ink"
        onClick={() => {
          if (!isTauri()) return
          void invoke<string>("ai_wrapped", {
            period: range,
            minutes: summary.minutes,
            artist: summary.artists[0]?.label ?? "",
            track: summary.tracks[0]?.label ?? "",
          }).then(setWrapped)
        }}
      >
        {t("retro_wrapped")}
      </button>
      {wrapped ? <p className="mt-3 text-[15px] leading-relaxed">{wrapped}</p> : null}
      {summary.tracks.length === 0 && summary.artists.length === 0 ? (
        <p className="px-1 py-8 text-[14px] leading-relaxed text-muted">{t("retro_empty")}</p>
      ) : (
        <div className="mt-5 min-h-0 flex-1 space-y-6 overflow-auto pb-4">
          <Rank title={t("retro_top_tracks")} rows={summary.tracks} top={top} plays={(n) => t("retro_plays", { n })} />
          <Rank title={t("retro_top_artists")} rows={summary.artists} top={top} plays={(n) => t("retro_plays", { n })} />
        </div>
      )}
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl bg-soft px-3 py-3">
      <p className="text-[11px] uppercase tracking-wide text-muted">{label}</p>
      <p className="mt-1 text-[18px] font-medium tracking-tight">{value}</p>
    </div>
  )
}

function Rank({
  title,
  rows,
  top,
  plays,
}: {
  title: string
  rows: { key: string; label: string; detail: string; plays: number; ms: number }[]
  top: number
  plays: (n: number) => string
}) {
  if (!rows.length) return null
  return (
    <section>
      <h3 className="px-1 text-[13px] text-muted">{title}</h3>
      <ol className="mt-2 space-y-2">
        {rows.map((row, index) => (
          <li key={row.key} className="px-1">
            <div className="flex items-baseline justify-between gap-3">
              <span className="min-w-0 truncate text-[14px]">
                <span className="mr-2 tabular-nums text-muted">{index + 1}</span>
                {row.label}
              </span>
              <span className="shrink-0 text-[12px] text-muted">{plays(row.plays)}</span>
            </div>
            {row.detail ? <p className="truncate pl-5 text-[12px] text-muted">{row.detail}</p> : null}
            <div className="mt-1 h-1 overflow-hidden rounded-full bg-soft">
              <div className="h-full rounded-full bg-ink/70" style={{ width: `${Math.max(8, (row.ms / top) * 100)}%` }} />
            </div>
          </li>
        ))}
      </ol>
    </section>
  )
}
