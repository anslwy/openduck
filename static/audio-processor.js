const PLAYBACK_REFERENCE_STATE_KEY = "__openduckPlaybackReferenceState";
const PLAYBACK_REFERENCE_CANDIDATE_DELAYS = [
  512,
  1024,
  1536,
  2048,
  3072,
  4096,
];
const MIN_REFERENCE_ENERGY = 1e-8;
const MIN_MIC_ENERGY = 1e-8;
// Slightly above the Rust silence window so a muted turn can still flush.
const POST_MUTE_DRAIN_QUANTA = 160;

function getPlaybackReferenceState() {
  if (!globalThis[PLAYBACK_REFERENCE_STATE_KEY]) {
    globalThis[PLAYBACK_REFERENCE_STATE_KEY] = {
      ringBuffer: new Float32Array(1 << 18),
      writeIndex: 0,
    };
  }

  return globalThis[PLAYBACK_REFERENCE_STATE_KEY];
}

function readDelayedPlaybackSamples(length, delaySamples) {
  const { ringBuffer, writeIndex } = getPlaybackReferenceState();
  const delayedSamples = new Float32Array(length);
  const ringLength = ringBuffer.length;
  let readIndex = writeIndex - delaySamples - length;

  while (readIndex < 0) {
    readIndex += ringLength;
  }

  for (let index = 0; index < length; index += 1) {
    delayedSamples[index] = ringBuffer[(readIndex + index) % ringLength];
  }

  return delayedSamples;
}

function sumSquares(samples) {
  let total = 0;
  for (let index = 0; index < samples.length; index += 1) {
    const sample = samples[index];
    total += sample * sample;
  }
  return total;
}

function dotProduct(left, right) {
  let total = 0;
  for (let index = 0; index < left.length; index += 1) {
    total += left[index] * right[index];
  }
  return total;
}

function selectPlaybackReference(inputSamples) {
  const micEnergy = sumSquares(inputSamples);
  if (micEnergy <= MIN_MIC_ENERGY) {
    return { playbackActive: false, playbackReferenceData: null };
  }

  let bestReference = null;
  let bestScore = 0;

  for (const delaySamples of PLAYBACK_REFERENCE_CANDIDATE_DELAYS) {
    const candidateReference = readDelayedPlaybackSamples(
      inputSamples.length,
      delaySamples,
    );
    const referenceEnergy = sumSquares(candidateReference);
    if (referenceEnergy <= MIN_REFERENCE_ENERGY) {
      continue;
    }

    const score =
      Math.abs(dotProduct(inputSamples, candidateReference)) /
      Math.sqrt(micEnergy * referenceEnergy);

    if (score > bestScore) {
      bestScore = score;
      bestReference = candidateReference;
    }
  }

  return {
    playbackActive: bestReference !== null,
    playbackReferenceData: bestReference,
  };
}

class AudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.muted = false;
    this.postMuteDrainQuantaRemaining = 0;

    this.port.onmessage = (event) => {
      if (event.data?.type !== "set-muted") {
        return;
      }

      this.muted = Boolean(event.data.muted);
      this.postMuteDrainQuantaRemaining = this.muted
        ? POST_MUTE_DRAIN_QUANTA
        : 0;
    };
  }

  process(inputs) {
    const input = inputs[0];
    if (input && input.length > 0) {
      const inputChannelData = input[0];
      if (inputChannelData.length > 0) {
        if (this.muted && this.postMuteDrainQuantaRemaining <= 0) {
          return true;
        }

        const inputData = this.muted
          ? new Float32Array(inputChannelData.length)
          : new Float32Array(inputChannelData);

        if (this.muted) {
          this.postMuteDrainQuantaRemaining = Math.max(
            0,
            this.postMuteDrainQuantaRemaining - 1,
          );
        }

        const { playbackActive, playbackReferenceData } =
          selectPlaybackReference(inputData);
        const message = {
          inputData,
          playbackActive,
        };
        const transfer = [inputData.buffer];

        if (playbackReferenceData) {
          message.playbackReferenceData = playbackReferenceData;
          transfer.push(playbackReferenceData.buffer);
        }

        this.port.postMessage(message, transfer);
      }
    }

    return true;
  }
}

registerProcessor("audio-processor", AudioProcessor);
