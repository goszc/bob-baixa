const BUTTON_ID = "bob-baixa-btn"

function videoUrl() {
  try {
    const url = new URL(location.href)
    const id = url.searchParams.get("v")
    if (!id || !url.pathname.startsWith("/watch")) return null
    return "https://www.youtube.com/watch?v=" + id
  } catch {
    return null
  }
}

function ensureButton() {
  const url = videoUrl()
  const existing = document.getElementById(BUTTON_ID)
  if (!url) {
    existing?.remove()
    return
  }
  if (existing) return
  const button = document.createElement("button")
  button.id = BUTTON_ID
  button.type = "button"
  button.textContent = "baixar com bob baixa"
  button.style.cssText = [
    "position:fixed",
    "right:20px",
    "bottom:20px",
    "z-index:2147483646",
    "border:0",
    "border-radius:999px",
    "padding:10px 16px",
    "background:#111",
    "color:#f7f7f5",
    "font:500 14px/1 Outfit,Segoe UI,sans-serif",
    "letter-spacing:-0.01em",
    "cursor:pointer",
    "box-shadow:0 8px 24px rgba(0,0,0,.18)",
  ].join(";")
  button.addEventListener("click", () => {
    const current = videoUrl()
    if (!current) return
    button.disabled = true
    chrome.runtime.sendMessage({ type: "download", url: current }, (response) => {
      button.textContent = response && response.ok ? "na fila" : "abra o bob baixa"
      window.setTimeout(() => {
        button.textContent = "baixar com bob baixa"
        button.disabled = false
      }, 1800)
    })
  })
  document.documentElement.appendChild(button)
}

ensureButton()
new MutationObserver(ensureButton).observe(document.documentElement, { childList: true, subtree: true })
