const status = document.getElementById("status")
const open = document.getElementById("open")

fetch("http://127.0.0.1:47821/v1/health")
  .then((response) => response.json())
  .then(() => {
    status.textContent = "o app está aberto."
  })
  .catch(() => {
    status.textContent = "abra o bob baixa para baixar."
  })

open.addEventListener("click", async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
  const url = tab && tab.url
  if (!url || !url.includes("youtube.com/watch")) {
    status.textContent = "abra um vídeo do youtube."
    return
  }
  chrome.runtime.sendMessage({ type: "download", url }, (response) => {
    status.textContent = response && response.ok ? "na fila." : "não consegui falar com o app."
  })
})
