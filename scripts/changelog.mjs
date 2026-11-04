export function changelogSection(markdown, version) {
  const wanted = String(version).trim().replace(/^v/, "")
  if (!wanted) return ""
  let collecting = false
  const lines = []
  for (const line of markdown.split(/\r?\n/)) {
    const trimmed = line.trim()
    if (trimmed.startsWith("## ")) {
      if (collecting) break
      const title = trimmed.slice(3).trim().replace(/^v/, "").split(/\s+/)[0]
      collecting = title === wanted
      if (collecting) lines.push(line)
      continue
    }
    if (collecting) lines.push(line)
  }
  return lines.join("\n").trim()
}
