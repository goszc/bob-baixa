import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

const driveClient = process.env.BOB_GOOGLE_CLIENT_ID?.trim() ?? ""

export default defineConfig({
  define: {
    __BOB_DRIVE__: JSON.stringify(driveClient.length > 0),
  },
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 1420,
    strictPort: true,
  },
  preview: {
    host: "0.0.0.0",
    port: 1420,
    strictPort: true,
  },
})
