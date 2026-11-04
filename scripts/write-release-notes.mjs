import { readFileSync, writeFileSync } from "node:fs"
import { changelogSection } from "./changelog.mjs"

const version = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version
const changelog = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8")
const notes = changelogSection(changelog, version)
if (!notes) {
  console.error(`changelog sem a seção ${version}`)
  process.exit(1)
}
writeFileSync(new URL("../packaging/release-notes.md", import.meta.url), `${notes}\n`)
console.log("release-notes", version)
