import { useEffect, useState } from "react"
import { invoke, isTauri } from "../lib/api"
import type { TrackLyrics } from "../lib/player"
import { seek } from "../lib/player"
import { useT } from "../store"
import { Button } from "./ui/button"
import { Input } from "./ui/input"

interface Choice {
  source: string
  title: string
  artist: string
  album: string
  duration?: number
  score: number
  synced: boolean
  preview: string
  lrc: string
}

export function FindLyrics({
  path,
  title,
  artist,
  duration,
  onClose,
  onPinned,
}: {
  path: string
  title: string
  artist: string
  duration?: number
  onClose: () => void
  onPinned: () => void
}) {
  const t = useT()
  const [nextTitle, setTitle] = useState(title)
  const [nextArtist, setArtist] = useState(artist)
  const [choices, setChoices] = useState<Choice[]>([])
  const [busy, setBusy] = useState(false)
  const [note, setNote] = useState("")

  const search = async () => {
    if (!isTauri()) {
      setChoices([
        {
          source: "lrclib",
          title: nextTitle || "Canção",
          artist: nextArtist || "Banda",
          album: "Disco",
          score: 90,
          synced: true,
          preview: "primeira linha\nsegunda linha",
          lrc: "[00:01.00]primeira linha\n[00:05.00]segunda linha\n[00:09.00]terceira linha\n[00:13.00]quarta linha",
        },
      ])
      return
    }
    setBusy(true)
    setNote("")
    try {
      const found = await invoke<Choice[]>("search_lyrics", {
        title: nextTitle,
        artist: nextArtist,
        album: "",
        duration: duration ?? null,
      })
      setChoices(found)
      if (!found.length) setNote(t("lyrics_empty"))
    } catch {
      setNote(t("lyrics_empty"))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-ink/25 p-4" onClick={onClose}>
      <div className="max-h-[80vh] w-full max-w-[560px] overflow-auto rounded-2xl border border-line bg-surface p-5" onClick={(event) => event.stopPropagation()}>
        <h2 className="text-[16px] font-medium">{t("lyrics_find")}</h2>
        <p className="mt-1 text-[13px] text-muted">{t("lyrics_find_hint")}</p>
        <div className="mt-4 grid gap-2">
          <Input value={nextArtist} onChange={(event) => setArtist(event.target.value)} placeholder={t("player_unknown_artist")} />
          <Input value={nextTitle} onChange={(event) => setTitle(event.target.value)} placeholder={t("player_now")} />
          <Button onClick={() => void search()} disabled={busy}>
            {busy ? t("searching") : t("search_btn")}
          </Button>
        </div>
        {note ? <p className="mt-3 text-[13px] text-muted">{note}</p> : null}
        <ul className="mt-4 space-y-3">
          {choices.map((choice) => (
            <li key={`${choice.source}-${choice.title}-${choice.artist}`} className="rounded-xl border border-line p-3">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-[14px] font-medium">{choice.title}</div>
                  <div className="text-[12px] text-muted">
                    {choice.artist} · {choice.source} · {choice.score}
                  </div>
                </div>
                <Button
                  variant="outline"
                  onClick={() => {
                    void invoke("pin_lyrics", { path, lrc: choice.lrc })
                      .then(() => {
                        setNote(t("lyrics_pinned"))
                        onPinned()
                      })
                      .catch(() => setNote(t("player_failed")))
                  }}
                >
                  {t("lyrics_pin")}
                </Button>
              </div>
              <pre className="mt-2 whitespace-pre-wrap text-[13px] text-muted">{choice.preview}</pre>
            </li>
          ))}
        </ul>
      </div>
    </div>
  )
}

export function Karaoke({ lyrics, time, onClose }: { lyrics: TrackLyrics; time: number; onClose: () => void }) {
  const t = useT()
  const lines = lyrics.lines
  let current = 0
  if (lyrics.synced) {
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].at <= time + 0.05) current = index
    }
  }
  const upcoming = lines.slice(current + 1, current + 4)
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose()
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [onClose])
  return (
    <div className="fixed inset-0 z-40 flex flex-col bg-black text-white">
      <button type="button" className="absolute right-6 top-6 text-[13px] text-white/70" onClick={onClose}>
        {t("lyrics_karaoke_exit")}
      </button>
      <div className="flex flex-1 flex-col items-center justify-center px-8 text-center">
        <button type="button" className="max-w-[900px] text-[42px] font-medium leading-tight tracking-[-0.04em]" onClick={() => lyrics.synced && seek(lines[current]?.at ?? 0)}>
          {lines[current]?.text || t("lyrics_empty")}
        </button>
        <div className="mt-8 space-y-3 text-[20px] text-white/55">
          {upcoming.map((line, index) => (
            <button key={`${line.at}-${index}`} type="button" className="block w-full" onClick={() => lyrics.synced && seek(line.at)}>
              {line.text}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
