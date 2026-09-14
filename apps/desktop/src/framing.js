'use strict';

// Mirrors crates/automed/src/ipc/framing.rs exactly: a JSON-RPC frame is a
// 4-byte big-endian length prefix followed by that many bytes of UTF-8 JSON.
// Kept dependency-free (no Electron API) so it can be unit-tested and
// reused from both Main and any future non-Electron tooling.

const LENGTH_PREFIX_BYTES = 4;

function encodeFrame(payload) {
  const body = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
  const header = Buffer.alloc(LENGTH_PREFIX_BYTES);
  header.writeUInt32BE(body.length, 0);
  return Buffer.concat([header, body]);
}

// Incremental frame decoder for a byte stream (child.stdout 'data' events
// deliver arbitrary chunk boundaries, not one frame per chunk). Push bytes
// in with `push`; each complete frame is emitted as a Buffer via `onFrame`.
class FrameDecoder {
  constructor(onFrame, maxFrameLen) {
    this._onFrame = onFrame;
    this._maxFrameLen = maxFrameLen;
    this._buffer = Buffer.alloc(0);
  }

  push(chunk) {
    this._buffer = Buffer.concat([this._buffer, chunk]);
    for (;;) {
      if (this._buffer.length < LENGTH_PREFIX_BYTES) return;
      const len = this._buffer.readUInt32BE(0);
      if (this._maxFrameLen !== undefined && len > this._maxFrameLen) {
        throw new Error(`frame length ${len} exceeds max ${this._maxFrameLen}`);
      }
      const total = LENGTH_PREFIX_BYTES + len;
      if (this._buffer.length < total) return;
      const body = this._buffer.subarray(LENGTH_PREFIX_BYTES, total);
      this._buffer = this._buffer.subarray(total);
      this._onFrame(body);
    }
  }
}

module.exports = { encodeFrame, FrameDecoder, LENGTH_PREFIX_BYTES };
