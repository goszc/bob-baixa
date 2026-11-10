export interface LyricStamp {
  at: number
  text: string
}

/** Positive nudge delays the highlight so the line waits for the vocal. */
export function lyricClock(position: number, nudge = 0) {
  return Math.max(0, position - nudge)
}

export function activeLine(lines: LyricStamp[], position: number) {
  let found = -1
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].at <= position + 0.05) found = index
    else break
  }
  return found
}
