import assert from "node:assert/strict"
import test from "node:test"
import { fileIdFrom, mediaResponseInit, parseByteRange } from "./stream.js"

test("a stream url keeps only the file id", () => {
  assert.equal(fileIdFrom("/bob-baixa/ouvir/stream/abc_1"), "abc_1")
  assert.equal(fileIdFrom("/stream/"), "")
  assert.equal(fileIdFrom("/ouvir/app.js"), "")
})

test("a drive 206 stays a ranged response without reading the whole body", () => {
  const headers = new Headers({
    "content-type": "audio/ogg; charset=binary",
    "content-range": "bytes 0-1/4000",
    "content-length": "2",
  })
  const init = mediaResponseInit(206, headers, "bytes=0-1", 4000)
  assert.equal(init.status, 206)
  assert.equal(init.headers.get("Content-Range"), "bytes 0-1/4000")
  assert.equal(init.headers.get("Content-Length"), "2")
  assert.equal(init.headers.get("Content-Type"), "audio/ogg")
  assert.equal(init.headers.get("Accept-Ranges"), "bytes")
})

test("a missing content-range is rebuilt from the request and the known size", () => {
  const parsed = parseByteRange("bytes=100-", 4000)
  assert.deepEqual(parsed, { start: 100, end: 3999 })
  const init = mediaResponseInit(206, new Headers({ "content-type": "audio/wav" }), "bytes=0-1", 4000)
  assert.equal(init.headers.get("Content-Range"), "bytes 0-1/4000")
  assert.equal(init.headers.get("Content-Length"), "2")
})
