const PLAYBACK_REFERENCE_STATE_KEY = "__openduckPlaybackReferenceState";

function getPlaybackReferenceState() {
  if (!globalThis[PLAYBACK_REFERENCE_STATE_KEY]) {
    globalThis[PLAYBACK_REFERENCE_STATE_KEY] = {
      ringBuffer: new Float32Array(1 << 18),
      writeIndex: 0,
    };
  }

  return globalThis[PLAYBACK_REFERENCE_STATE_KEY];
}

function writePlaybackSamples(samples) {
  const state = getPlaybackReferenceState();
  const { ringBuffer } = state;

  for (let index = 0; index < samples.length; index += 1) {
    ringBuffer[state.writeIndex] = samples[index];
    state.writeIndex = (state.writeIndex + 1) % ringBuffer.length;
  }
}

class PlaybackProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = [];
    this.offset = 0;
    this.bufferedSamples = 0;
    this.prebufferSamples = 0;
    this.started = false;
    this.currentRequestId = null;

    this.port.onmessage = (event) => {
      const { type } = event.data ?? {};

      if (type === "stop") {
        this.reset();
        return;
      }

      if (type !== "push") {
        return;
      }

      const { requestId, samples, prebufferSamples } = event.data;
      if (this.currentRequestId !== requestId) {
        this.reset();
        this.currentRequestId = requestId;
      }

      const chunk =
        samples instanceof Float32Array ? samples : new Float32Array(samples);
      if (chunk.length === 0) {
        return;
      }

      this.queue.push({ requestId, samples: chunk });
      this.bufferedSamples += chunk.length;
      this.prebufferSamples = Math.max(
        0,
        prebufferSamples ?? this.prebufferSamples,
      );

      if (!this.started && this.bufferedSamples >= this.prebufferSamples) {
        this.started = true;
      }
    };
  }

  reset() {
    this.queue = [];
    this.offset = 0;
    this.bufferedSamples = 0;
    this.started = false;
    this.currentRequestId = null;
  }

  process(inputs, outputs) {
    const output = outputs[0]?.[0];
    if (!output) {
      return true;
    }

    output.fill(0);

    if (
      !this.started &&
      this.bufferedSamples >= this.prebufferSamples &&
      this.bufferedSamples > 0
    ) {
      this.started = true;
    }

    if (this.started) {
      let writeIndex = 0;
      while (writeIndex < output.length) {
        const head = this.queue[0];
        if (!head) {
          this.started = false;
          break;
        }

        const available = head.samples.length - this.offset;
        const toCopy = Math.min(available, output.length - writeIndex);
        output.set(
          head.samples.subarray(this.offset, this.offset + toCopy),
          writeIndex,
        );
        this.offset += toCopy;
        writeIndex += toCopy;
        this.bufferedSamples -= toCopy;

        if (this.offset >= head.samples.length) {
          this.queue.shift();
          this.offset = 0;
          this.port.postMessage({
            type: "chunk-finished",
            requestId: head.requestId,
          });
        }
      }
    }

    writePlaybackSamples(output);
    return true;
  }
}

registerProcessor("playback-processor", PlaybackProcessor);
