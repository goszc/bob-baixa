chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (!message || message.type !== "download" || !message.url) return
  fetch("http://127.0.0.1:47821/v1/download", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ url: message.url }),
  })
    .then((response) => response.json())
    .then((data) => sendResponse(data))
    .catch(async () => {
      const link = "bobbaixa://download?url=" + encodeURIComponent(message.url)
      try {
        await chrome.tabs.create({ url: link, active: false })
        sendResponse({ ok: true, launched: true })
      } catch (error) {
        sendResponse({ ok: false, error: String(error) })
      }
    })
  return true
})
