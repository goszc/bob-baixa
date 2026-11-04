export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
}

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core")
  return invoke<T>(command, args)
}

export async function listenBob<T>(handler: (payload: T) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event")
  return listen<T>("bob", (event) => handler(event.payload))
}

export async function applyWindowTheme(theme: "system" | "light" | "dark") {
  if (!isTauri()) return
  const { getCurrentWindow } = await import("@tauri-apps/api/window")
  const win = getCurrentWindow()
  await win.setTheme(theme === "system" ? null : theme)
}

export async function chooseFolder(): Promise<string | null> {
  if (!isTauri()) return null
  const { open } = await import("@tauri-apps/plugin-dialog")
  const picked = await open({ directory: true, multiple: false })
  return typeof picked === "string" ? picked : null
}

export async function chooseFile(): Promise<string | null> {
  if (!isTauri()) return null
  const { open } = await import("@tauri-apps/plugin-dialog")
  const picked = await open({
    multiple: false,
    filters: [
      {
        name: "media",
        extensions: ["mp4", "mkv", "webm", "mov", "avi", "m4v", "mp3", "m4a", "wav", "flac", "opus", "ogg"],
      },
    ],
  })
  return typeof picked === "string" ? picked : null
}

export async function revealPath(path: string) {
  if (!isTauri()) return
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener")
  await revealItemInDir(path)
}

export async function readTextFile(path: string): Promise<string> {
  return invoke<string>("read_text_file", { path })
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error
  if (error instanceof Error) return error.message
  return String(error)
}
