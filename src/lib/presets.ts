import type { Config, DownloadPreset } from "./types"

export const builtinPresets: DownloadPreset[] = [
  {
    id: "builtin-economy",
    name: "modo econômico",
    quality: "1080",
    container: "mp4",
    audioOnly: true,
    audioFormat: "opus",
    audioBitrate: "24k",
    subtitles: false,
    embedSubs: false,
    subLangs: "pt.*,en.*",
    embedThumbnail: false,
    embedMetadata: true,
    embedChapters: false,
    sponsorblock: false,
    sponsorCategories: [],
  },
  {
    id: "builtin-mp3",
    name: "música mp3",
    quality: "1080",
    container: "mp4",
    audioOnly: true,
    audioFormat: "mp3",
    audioBitrate: "192k",
    subtitles: false,
    embedSubs: false,
    subLangs: "pt.*,en.*",
    embedThumbnail: true,
    embedMetadata: true,
    embedChapters: false,
    sponsorblock: false,
    sponsorCategories: [],
  },
  {
    id: "builtin-4k",
    name: "vídeo 4k",
    quality: "2160",
    container: "mp4",
    audioOnly: false,
    audioFormat: "mp3",
    audioBitrate: "192k",
    subtitles: true,
    embedSubs: true,
    subLangs: "pt.*,en.*",
    embedThumbnail: true,
    embedMetadata: true,
    embedChapters: true,
    sponsorblock: false,
    sponsorCategories: ["sponsor", "intro", "outro", "selfpromo"],
  },
  {
    id: "builtin-1080",
    name: "vídeo 1080",
    quality: "1080",
    container: "mp4",
    audioOnly: false,
    audioFormat: "mp3",
    audioBitrate: "192k",
    subtitles: false,
    embedSubs: true,
    subLangs: "pt.*,en.*",
    embedThumbnail: true,
    embedMetadata: true,
    embedChapters: true,
    sponsorblock: false,
    sponsorCategories: ["sponsor", "intro", "outro", "selfpromo"],
  },
]

export function presetPatch(preset: DownloadPreset): Partial<Config> {
  return {
    quality: preset.quality,
    container: preset.container,
    audioOnly: preset.audioOnly,
    audioFormat: preset.audioFormat,
    audioBitrate: preset.audioBitrate,
    subtitles: preset.subtitles,
    embedSubs: preset.embedSubs,
    subLangs: preset.subLangs,
    embedThumbnail: preset.embedThumbnail,
    embedMetadata: preset.embedMetadata,
    embedChapters: preset.embedChapters,
    sponsorblock: preset.sponsorblock,
    sponsorCategories: [...preset.sponsorCategories],
  }
}

export function configMatchesPreset(config: Config, preset: DownloadPreset) {
  const categories = [...config.sponsorCategories].sort().join("\n")
  const presetCategories = [...preset.sponsorCategories].sort().join("\n")
  return (
    config.quality === preset.quality &&
    config.container === preset.container &&
    config.audioOnly === preset.audioOnly &&
    config.audioFormat === preset.audioFormat &&
    config.audioBitrate === preset.audioBitrate &&
    config.subtitles === preset.subtitles &&
    config.embedSubs === preset.embedSubs &&
    config.subLangs === preset.subLangs &&
    config.embedThumbnail === preset.embedThumbnail &&
    config.embedMetadata === preset.embedMetadata &&
    config.embedChapters === preset.embedChapters &&
    config.sponsorblock === preset.sponsorblock &&
    categories === presetCategories
  )
}

export function presetFromConfig(config: Config, id: string, name: string): DownloadPreset {
  return {
    id,
    name,
    quality: config.quality,
    container: config.container,
    audioOnly: config.audioOnly,
    audioFormat: config.audioFormat,
    audioBitrate: config.audioBitrate,
    subtitles: config.subtitles,
    embedSubs: config.embedSubs,
    subLangs: config.subLangs,
    embedThumbnail: config.embedThumbnail,
    embedMetadata: config.embedMetadata,
    embedChapters: config.embedChapters,
    sponsorblock: config.sponsorblock,
    sponsorCategories: [...config.sponsorCategories],
  }
}
