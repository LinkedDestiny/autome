import { encodeMessage, type LogChunk } from './eventbus-protocol.ts';

const MAX_MESSAGE_BYTES = 65536;

// Return one UTF-8 scalar (or one maximal invalid prefix). A valid but unfinished
// prefix is retained until the next read, unless this is the final read.
function unitLength(bytes: Buffer, at: number, final: boolean): number {
  const head = bytes[at];
  const size = head >= 0xc2 && head <= 0xdf ? 2
    : head >= 0xe0 && head <= 0xef ? 3 : head >= 0xf0 && head <= 0xf4 ? 4 : 1;
  for (let i = 1; i < size; i++) {
    if (at + i === bytes.length) return final ? i : 0;
    const byte = bytes[at + i];
    const min = i === 1 && head === 0xe0 ? 0xa0 : i === 1 && head === 0xf0 ? 0x90 : 0x80;
    const max = i === 1 && head === 0xed ? 0x9f : i === 1 && head === 0xf4 ? 0x8f : 0xbf;
    if (byte < min || byte > max) return i;
  }
  return size;
}

export function splitChunks({ sessionId, start, bytes, final }: {
  sessionId: string; start: number; bytes: Buffer; final: boolean;
}): { messages: LogChunk[]; consumed: number } {
  const messages: LogChunk[] = [];
  let consumed = 0;
  let offset = start;
  let parts: string[] = [];
  let textBytes = 0;
  const envelope = (next: number, text: string): LogChunk => ({ type: 'log.chunk', session_id: sessionId, offset, next, text });
  const flush = () => {
    if (!parts.length) return;
    messages.push(envelope(start + consumed, parts.join('')));
    offset = start + consumed;
    parts = [];
    textBytes = 0;
  };
  while (consumed < bytes.length) {
    const size = unitLength(bytes, consumed, final);
    if (!size) break;
    const text = bytes.subarray(consumed, consumed + size).toString('utf8');
    const encodedBytes = Buffer.byteLength(JSON.stringify(text)) - 2;
    const next = start + consumed + size;
    if (textBytes + encodedBytes + Buffer.byteLength(encodeMessage(envelope(next, ''))) > MAX_MESSAGE_BYTES) flush();
    parts.push(text);
    textBytes += encodedBytes;
    consumed += size;
  }
  flush();
  return { messages, consumed };
}
