class AudioProcessor extends AudioWorkletProcessor {
  process(inputs, outputs, parameters) {
    const input = inputs[0];
    if (input && input.length > 0) {
      const inputChannelData = input[0];
      // Send the audio data to the main thread
      if (inputChannelData.length > 0) {
        this.port.postMessage(inputChannelData);
      }
    }
    // Return true to keep the processor alive
    return true;
  }
}

registerProcessor('audio-processor', AudioProcessor);