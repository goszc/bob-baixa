export const EQ_BANDS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]

export const EQ_PRESETS: Record<string, number[]> = {
  flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  graves: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  voz: [-2, -1, 0, 2, 4, 4, 2, 1, 0, 0],
  rock: [5, 4, 2, 0, -1, 1, 3, 4, 4, 4],
  pop: [-1, 2, 4, 4, 2, 0, 1, 2, 3, 3],
  classico: [4, 3, 2, 1, 0, 0, -1, -1, 2, 3],
  eletronico: [5, 4, 1, 0, -2, 0, 1, 3, 4, 5],
}

export function clampBand(value: number) {
  if (!Number.isFinite(value)) return 0
  return Math.max(-12, Math.min(12, Math.round(value)))
}

export function bandsFor(preset: string, custom: number[]) {
  const known = EQ_PRESETS[preset]
  if (known) return known.slice()
  return EQ_BANDS.map((_, index) => clampBand(custom[index] ?? 0))
}

export function eqIsFlat(bands: number[]) {
  return bands.every((band) => band === 0)
}
