const PLAYBACK_REFERENCE_STATE_KEY = "__openduckPlaybackReferenceState";
const LIP_SYNC_UPDATE_INTERVAL_FRAMES = 512;
const LIP_SYNC_RMS_FLOOR = 0.0015;
const LIP_SYNC_PEAK_FLOOR = 0.004;
const LIP_SYNC_RMS_GAIN = 15;
const LIP_SYNC_PEAK_GAIN = 3.5;
const LIP_SYNC_MIN_VOICED_LEVEL = 0.08;
const LIP_SYNC_ATTACK = 0.6;
const LIP_SYNC_RELEASE = 0.35;

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

function clamp01(value) {
  return Math.min(1, Math.max(0, value));
}

function lipSyncTargetFromEnergy(sumSquares, peak, sampleCount) {
  if (sampleCount <= 0) {
    return 0;
  }

  const rms = Math.sqrt(sumSquares / sampleCount);
  const rmsLevel = Math.max(0, rms - LIP_SYNC_RMS_FLOOR) * LIP_SYNC_RMS_GAIN;
  const peakLevel =
    Math.max(0, peak - LIP_SYNC_PEAK_FLOOR) * LIP_SYNC_PEAK_GAIN;
  const voiceFloor =
    rms > LIP_SYNC_RMS_FLOOR || peak > LIP_SYNC_PEAK_FLOOR
      ? LIP_SYNC_MIN_VOICED_LEVEL
      : 0;

  return clamp01(Math.max(rmsLevel, peakLevel, voiceFloor));
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
    this.chunkStarted = false;
    this.lipSyncEnvelope = 0;
    this.lipSyncFramesUntilPost = 0;

    this.port.onmessage = (event) => {
      const { type } = event.data ?? {};

      if (type === "stop") {
        this.reset();
        return;
      }

      if (type !== "push") {
        return;
      }

      const { requestId, samples, prebufferSamples, isNewSegment, lipSync } =
        event.data;
      if (this.currentRequestId !== requestId) {
        this.reset();
        this.currentRequestId = requestId;
      }

      const chunk =
        samples instanceof Float32Array ? samples : new Float32Array(samples);
      if (chunk.length === 0) {
        return;
      }

      this.queue.push({
        requestId,
        samples: chunk,
        isNewSegment,
        lipSync: lipSync !== false,
      });
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
    this.chunkStarted = false;
    this.lipSyncEnvelope = 0;
    this.lipSyncFramesUntilPost = 0;
    this.port.postMessage({ type: "lip-sync", level: 0 });
  }

  updateLipSyncLevel(target, frameCount) {
    const smoothing =
      target > this.lipSyncEnvelope ? LIP_SYNC_ATTACK : LIP_SYNC_RELEASE;
    this.lipSyncEnvelope += (target - this.lipSyncEnvelope) * smoothing;
    if (this.lipSyncEnvelope < 0.01 && target === 0) {
      this.lipSyncEnvelope = 0;
    }

    this.lipSyncFramesUntilPost -= frameCount;
    if (this.lipSyncFramesUntilPost <= 0) {
      this.lipSyncFramesUntilPost += LIP_SYNC_UPDATE_INTERVAL_FRAMES;
      this.port.postMessage({
        type: "lip-sync",
        level: this.lipSyncEnvelope,
      });
    }
  }

  process(inputs, outputs) {
    const output = outputs[0]?.[0];
    if (!output) {
      return true;
    }

    output.fill(0);
    let lipSyncSumSquares = 0;
    let lipSyncPeak = 0;

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

        if (this.offset === 0 && !this.chunkStarted) {
          this.chunkStarted = true;
          if (head.isNewSegment !== false) {
            this.port.postMessage({
              type: "chunk-started",
              requestId: head.requestId,
            });
          }
        }

        const available = head.samples.length - this.offset;
        const toCopy = Math.min(available, output.length - writeIndex);
        output.set(
          head.samples.subarray(this.offset, this.offset + toCopy),
          writeIndex,
        );
        if (head.lipSync) {
          for (let index = 0; index < toCopy; index += 1) {
            const sample = head.samples[this.offset + index];
            lipSyncSumSquares += sample * sample;
            lipSyncPeak = Math.max(lipSyncPeak, Math.abs(sample));
          }
        }
        this.offset += toCopy;
        writeIndex += toCopy;
        this.bufferedSamples -= toCopy;

        if (this.offset >= head.samples.length) {
          this.queue.shift();
          this.offset = 0;
          this.chunkStarted = false;
          this.port.postMessage({
            type: "chunk-finished",
            requestId: head.requestId,
          });
        }
      }
    }

    this.updateLipSyncLevel(
      lipSyncTargetFromEnergy(
        lipSyncSumSquares,
        lipSyncPeak,
        output.length,
      ),
      output.length,
    );
    writePlaybackSamples(output);
    return true;
  }
}

registerProcessor("playback-processor", PlaybackProcessor);
