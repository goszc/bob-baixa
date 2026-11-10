import { readFileSync, writeFileSync } from "node:fs"

const file = process.argv[2]
const id = (process.argv[3] || "").trim()
if (!file) {
  console.error("uso: node scripts/inject-web-client.mjs <config.js> <client id>")
  process.exit(1)
}
if (!/^[\w.-]+\.apps\.googleusercontent\.com$/.test(id)) {
  console.error("BOB_GOOGLE_WEB_CLIENT_ID não parece um client id web.")
  process.exit(1)
}
const text = readFileSync(file, "utf8")
if (!text.includes("clientId:")) {
  console.error("config.js não tem clientId.")
  process.exit(1)
}
writeFileSync(file, text.replace(/clientId:\s*"[^"]*"/, `clientId: ${JSON.stringify(id)}`))
