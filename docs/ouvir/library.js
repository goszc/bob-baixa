/** Files the desktop app uploaded, seen with the same project's drive.file scope. */

export const DRIVE_SCOPE = "https://www.googleapis.com/auth/drive.file"

const KINDS = ["audio", "video", "lyrics", "cover"]

export function libraryQuery() {
  const kinds = KINDS.map((kind) => `appProperties has { key='bobKind' and value='${kind}' }`)
  return `trashed = false and (${kinds.join(" or ")})`
}

export function libraryFromFiles(files) {
  const list = Array.isArray(files) ? files : []
  const props = (file) => file?.appProperties || {}
  return list
    .filter((file) => props(file).bobKind === "audio" || props(file).bobKind === "video")
    .map((file) => {
      const meta = props(file)
      const related = (kind) => list.find((item) => props(item).bobKind === kind && props(item).bobMedia === file.id)
      return {
        id: file.id,
        name: file.name || "",
        title: meta.bobTitle || strip(file.name || ""),
        artist: meta.bobArtist || "",
        album: meta.bobAlbum || "",
        video: meta.bobKind === "video",
        lyricsId: related("lyrics")?.id || "",
        coverId: related("cover")?.id || "",
      }
    })
}

function strip(name) {
  return name.replace(/\.[a-z0-9]+$/i, "")
}
