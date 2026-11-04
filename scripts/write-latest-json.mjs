import { readFileSync, writeFileSync } from "node:fs"

const version = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version
const signature = readFileSync(new URL("../packaging/bob-baixa-setup.exe.sig", import.meta.url), "utf8").trim()
const repo = process.env.GITHUB_REPOSITORY
const url = process.env.UPDATE_ASSET_URL
  || (repo ? `https://github.com/${repo}/releases/download/v${version}/bob-baixa-setup.exe` : "")

if (!url) {
  console.error("defina UPDATE_ASSET_URL ou rode dentro do github actions")
  process.exit(1)
}

const manifest = {
  version,
  notes: "bob baixa",
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": { signature, url },
  },
}

writeFileSync(new URL("../packaging/latest.json", import.meta.url), JSON.stringify(manifest, null, 2))
console.log("latest.json", version)
