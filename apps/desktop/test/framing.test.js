'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { encodeFrame, FrameDecoder } = require('../src/framing');

test('decoder yields nothing until a frame is complete', () => {
  const full = encodeFrame(Buffer.from('payload'));
  const frames = [];
  const decoder = new FrameDecoder((f) => frames.push(f));
  decoder.push(full.subarray(0, 3));
  assert.deepEqual(frames, []);
  decoder.push(full.subarray(3));
  assert.deepEqual(frames.map(String), ['payload']);
});

test('decoder drains multiple concatenated frames in order', () => {
  const concatenated = Buffer.concat([encodeFrame(Buffer.from('one')), encodeFrame(Buffer.from('two'))]);
  const frames = [];
  const decoder = new FrameDecoder((f) => frames.push(f));
  decoder.push(concatenated);
  assert.deepEqual(frames.map(String), ['one', 'two']);
});

test('decoder rejects oversized length without waiting for payload', () => {
  const frames = [];
  const decoder = new FrameDecoder((f) => frames.push(f), 4);
  const header = Buffer.alloc(4);
  header.writeUInt32BE(10, 0);
  assert.throws(() => decoder.push(header), /exceeds max/);
});

test('decoder leaves a trailing partial frame buffered', () => {
  const frames = [];
  const decoder = new FrameDecoder((f) => frames.push(f));
  const complete = encodeFrame(Buffer.from('complete'));
  const partialHeader = Buffer.alloc(4);
  partialHeader.writeUInt32BE(12, 0);
  decoder.push(Buffer.concat([complete, partialHeader, Buffer.from('only-part')]));
  assert.deepEqual(frames.map(String), ['complete']);
  decoder.push(Buffer.from('ial'));
  assert.deepEqual(frames.map(String), ['complete', 'only-partial']);
});
