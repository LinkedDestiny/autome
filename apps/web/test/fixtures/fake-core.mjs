#!/usr/bin/env node
// A framed process with controllable reply ordering and exit, without a database.
import { FrameDecoder, encodeFrame } from '../../server/core/framing.ts';
const decoder = new FrameDecoder(frame => {
  const command = JSON.parse(frame.toString());
  if (command.method === 'exit') {
    process.exit(0);
  }
  setTimeout(() => {
    process.stdout.write(encodeFrame(JSON.stringify({
      frame: 'reply', request_id: command.request_id,
      command_id: command.command_id, outcome: { status: 'ok' },
    })));
  }, command.params?.delayMs ?? Number(process.env.FAKE_CORE_DELAY_MS ?? 0));
});
process.stdin.on('data', chunk => decoder.push(chunk));
process.stdin.on('end', () => process.exit(0));
