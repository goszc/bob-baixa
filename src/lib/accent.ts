export async function paintAccent(src: string) {
  const color = (await colorFrom(src)) || hashColor(src || "bob")
  document.documentElement.style.setProperty("--accent", color)
}

async function colorFrom(src: string) {
  if (!src || typeof Image === "undefined") return ""
  try {
    const image = new Image()
    image.crossOrigin = "anonymous"
    image.src = src
    await image.decode()
    const canvas = document.createElement("canvas")
    canvas.width = 8
    canvas.height = 8
    const context = canvas.getContext("2d", { willReadFrequently: true })
    if (!context) return ""
    context.drawImage(image, 0, 0, 8, 8)
    const data = context.getImageData(0, 0, 8, 8).data
    let r = 0
    let g = 0
    let b = 0
    const count = data.length / 4
    for (let index = 0; index < data.length; index += 4) {
      r += data[index]
      g += data[index + 1]
      b += data[index + 2]
    }
    return `rgb(${Math.round(r / count)}, ${Math.round(g / count)}, ${Math.round(b / count)})`
  } catch {
    return ""
  }
}

function hashColor(text: string) {
  let value = 0
  for (const char of text) value = (value * 33 + char.charCodeAt(0)) >>> 0
  const hue = value % 360
  return `hsl(${hue} 42% 62%)`
}
