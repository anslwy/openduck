# OpenDuck

OpenDuck is a local voice-call prototype built with a Svelte frontend and a Tauri/Rust backend.
The frontend captures microphone audio and plays streamed speech output.
The backend uses Gemma for transcription and reply generation, and a selectable speech backend for text-to-speech.

## Speech Models

The speech card in the app can switch between:

- `CSM Expressiva 1B`: the original MLX-based speech model, with optional quantization.
- `Kokoro-82M`: a lighter English TTS backend that runs through `mlx-audio` with the default `af_heart` voice from `mlx-community/Kokoro-82M-bf16`.

Use the dropdown in the speech card to choose the backend, then download and load that model before starting a call.
If you want to use Kokoro in a fresh checkout, run `scripts/setup_python_env.sh` first so the dedicated Kokoro environment installs `mlx-audio` plus the required English spaCy model.

## Conversation Flow

1. The user starts a call in `src/routes/+page.svelte`. The frontend resets the session, starts microphone capture, and sets the UI to `Listening`.
2. Audio chunks are sent to `receive_audio_chunk` in `src-tauri/src/lib.rs`. The backend uses simple voice activity detection to buffer speech and treat a long enough silence as the end of a turn.
3. The buffered audio is written to a temporary WAV file and sent to Gemma for transcription. Empty or filler-only transcripts are ignored.
4. A valid transcript interrupts any active reply, so the user can barge in while the assistant is speaking.
5. The backend asks Gemma for a short spoken reply using the system prompt plus recent conversation history.
6. The reply is sanitized, split into short spoken segments, and sent to the selected speech worker for text-to-speech generation.
7. The frontend listens for `csm-audio-start`, `csm-audio-chunk`, `csm-audio-done`, and `call-stage` events, queues the generated audio, plays it sequentially, and updates the visible call state.
8. Successful user and assistant turns are stored in memory with a rolling limit of 24 turns. Starting or ending a call clears that history and resets the session.

## Flowchart

```mermaid
flowchart TD
    A[Start Call in +page.svelte] --> B[Reset call session]
    B --> C[Start mic capture]
    C --> D[Send audio chunks to receive_audio_chunk]
    D --> E{Speech detected?}
    E -- No --> D
    E -- Yes --> F[Buffer audio]
    F --> G{Long enough silence?}
    G -- No --> D
    G -- Yes --> H[Write temp WAV]
    H --> I[Transcribe with Gemma]
    I --> J{Meaningful transcript?}
    J -- No --> K[Return to Listening]
    J -- Yes --> L[Interrupt active reply]
    L --> M[Build prompt with system prompt and recent turns]
    M --> N[Generate reply with Gemma]
    N --> O[Sanitize and split into short spoken segments]
    O --> P[Send segments to selected speech worker]
    P --> Q[Emit audio and call-stage events]
    Q --> R[Frontend queues and plays audio]
    R --> S[Store user and assistant turn in memory]
    S --> K
    K --> D
    T[End Call] --> U[Stop capture and playback]
    U --> B
```

## Key Files

- `src/routes/+page.svelte`: call UI, microphone capture, Tauri event listeners, playback queue, and call-stage state.
- `src-tauri/src/lib.rs`: voice activity detection, transcription, reply generation, conversation memory, and speech worker orchestration.
- `src-tauri/resources/csm_stream.py`: shared speech worker entrypoint for CSM Expressiva 1B and Kokoro-82M.
- `scripts/setup_python_env.sh`: bootstraps the Gemma environment plus separate CSM and Kokoro speech environments, including `mlx-audio` for Kokoro.
