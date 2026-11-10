import { EQ_BANDS, EQ_PRESETS, bandsFor, clampBand } from "../lib/eq"
import { activeEq, setEq, useMusic, type EqScope } from "../lib/music"
import { useT } from "../store"
import { Button } from "./ui/button"

const PRESET_KEYS = ["flat", "graves", "voz", "rock", "pop", "classico", "eletronico", "custom"] as const

export function Equalizer({ path, onClose }: { path?: string; onClose: () => void }) {
  const t = useT()
  const music = useMusic()
  const current = activeEq(path)
  const preset = current.preset
  const bands = bandsFor(preset, current.bands)
  const scope: EqScope = music.eqScope

  const apply = (nextPreset: string, nextBands: number[], nextScope = scope) => {
    setEq(nextPreset, nextBands, nextScope, path)
  }

  return (
    <div className="fixed inset-0 z-40 flex items-end justify-center bg-ink/25 p-4 sm:items-center" onClick={onClose}>
      <div className="w-full max-w-[560px] rounded-2xl border border-line bg-surface p-5" onClick={(event) => event.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h2 className="text-[16px] font-medium">{t("eq_title")}</h2>
          <button type="button" className="text-[13px] text-muted" onClick={onClose}>
            {t("player_back")}
          </button>
        </div>
        <div className="mt-3 flex gap-2">
          <Button variant={scope === "global" ? "default" : "outline"} onClick={() => apply(preset, bands, "global")}>
            {t("eq_global")}
          </Button>
          <Button variant={scope === "track" ? "default" : "outline"} onClick={() => apply(preset, bands, "track")} disabled={!path}>
            {t("eq_track")}
          </Button>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          {PRESET_KEYS.map((id) => (
            <button
              key={id}
              type="button"
              onClick={() => apply(id, id === "custom" ? bands : EQ_PRESETS[id])}
              className={preset === id ? "rounded-full bg-fill px-3 py-1 text-[13px] text-onfill" : "rounded-full bg-soft px-3 py-1 text-[13px] text-muted"}
            >
              {t(`eq_${id}` as "eq_flat")}
            </button>
          ))}
        </div>
        <div className="mt-5 flex items-end justify-between gap-2">
          {EQ_BANDS.map((frequency, index) => (
            <label key={frequency} className="flex flex-1 flex-col items-center gap-2 text-[11px] text-muted">
              <input
                aria-label={`${frequency} Hz`}
                type="range"
                min={-12}
                max={12}
                value={bands[index] ?? 0}
                onChange={(event) => {
                  const next = bands.slice()
                  next[index] = clampBand(Number(event.target.value))
                  apply("custom", next)
                }}
                className="h-28 accent-current [writing-mode:vertical-lr] [direction:rtl]"
              />
              <span>{frequency >= 1000 ? `${frequency / 1000}k` : frequency}</span>
            </label>
          ))}
        </div>
      </div>
    </div>
  )
}
