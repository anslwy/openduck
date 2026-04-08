<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';

	type CsmAudioStartEvent = {
		request_id: number;
		text: string;
		total_segments: number;
	};

	type CsmAudioChunkEvent = {
		request_id: number;
		audio_wav_base64: string;
	};

	type CsmAudioDoneEvent = {
		request_id: number;
	};

	type CsmAudioStopEvent = Record<string, never>;

	type CsmErrorEvent = {
		request_id?: number | null;
		message: string;
	};

	type CsmStatusEvent = {
		message: string;
	};

	type CallStageEvent = {
		phase: string;
		message: string;
	};

	type ModelDownloadProgressEvent = {
		model: 'gemma' | 'csm';
		phase: 'progress' | 'completed' | 'error';
		message: string;
		progress?: number | null;
		indeterminate: boolean;
	};

	type CsmVoiceOption = 'male' | 'female';

	let calling = $state(false);
	let micMuted = $state(false);
	let time = $state(0);
	let isGemmaDownloaded = $state(false);
	let isGemmaLoaded = $state(false);
	let isCsmDownloaded = $state(false);
	let isCsmLoaded = $state(false);
	let isDownloadingGemma = $state(false);
	let isLoadingGemma = $state(false);
	let isDownloadingCsm = $state(false);
	let isLoadingCsm = $state(false);
	let gemmaDownloadMessage = $state('Preparing download...');
	let gemmaDownloadProgress = $state<number | null>(null);
	let gemmaDownloadIndeterminate = $state(true);
	let csmDownloadMessage = $state('Preparing download...');
	let csmDownloadProgress = $state<number | null>(null);
	let csmDownloadIndeterminate = $state(true);
	let csmLoadMessage = $state('Starting worker...');
	let selectedCsmVoice = $state<CsmVoiceOption>('male');
	let lastAppliedCsmVoice = $state<CsmVoiceOption>('male');

	let captureContext: AudioContext | null = null;
	let mediaStream: MediaStream | null = null;
	let source: MediaStreamAudioSourceNode | null = null;
	let processor: AudioWorkletNode | null = null;
	let healthCheckInterval: ReturnType<typeof window.setInterval> | null = null;
	let callTimerInterval: ReturnType<typeof window.setInterval> | null = null;
	let playbackIdleTimeout: ReturnType<typeof window.setTimeout> | null = null;
	let eventUnlisteners: UnlistenFn[] = [];
	let activeTtsRequestId: number | null = null;
	let pendingTtsSegments = $state(0);
	let activePlaybackAudio: HTMLAudioElement | null = null;
	let activePlaybackUrl: string | null = null;
	let queuedPlaybackUrls: string[] = [];
	let callStagePhase = $state<'idle' | 'listening' | 'processing_audio' | 'thinking' | 'generating_audio' | 'speaking'>('idle');
	let callStageMessage = $state('');

	const formattedTime = $derived(
		`${Math.floor(time / 60)
			.toString()
			.padStart(2, '0')}:${(time % 60).toString().padStart(2, '0')}`
	);
	const modelsReady = $derived(isGemmaLoaded && isCsmLoaded);

	function setCallStage(
		phase: 'idle' | 'listening' | 'processing_audio' | 'thinking' | 'generating_audio' | 'speaking',
		message: string
	) {
		callStagePhase = phase;
		callStageMessage = message;
	}

	function resetDownloadState(model: 'gemma' | 'csm') {
		if (model === 'gemma') {
			gemmaDownloadMessage = 'Preparing download...';
			gemmaDownloadProgress = null;
			gemmaDownloadIndeterminate = true;
			return;
		}

		csmDownloadMessage = 'Preparing download...';
		csmDownloadProgress = null;
		csmDownloadIndeterminate = true;
	}

	function applyDownloadEvent(payload: ModelDownloadProgressEvent) {
		if (payload.model === 'gemma') {
			gemmaDownloadMessage = payload.message;
			gemmaDownloadProgress = payload.progress ?? null;
			gemmaDownloadIndeterminate = payload.indeterminate;
			return;
		}

		csmDownloadMessage = payload.message;
		csmDownloadProgress = payload.progress ?? null;
		csmDownloadIndeterminate = payload.indeterminate;
	}

	async function applyCsmVoiceSelection() {
		await invoke('set_csm_voice', { voice: selectedCsmVoice });
		lastAppliedCsmVoice = selectedCsmVoice;
	}

	async function handleCsmVoiceChange() {
		const attemptedVoice = selectedCsmVoice;

		try {
			await applyCsmVoiceSelection();
		} catch (err) {
			selectedCsmVoice = lastAppliedCsmVoice;
			console.error('Failed to update CSM voice:', err);
			alert(`Failed to switch CSM voice to ${attemptedVoice}.\n${String(err)}`);
		}
	}

	async function syncModelStatus() {
		try {
			const [gemmaDownloaded, gemmaLoaded, csmDownloaded, csmLoaded] = await Promise.all([
				invoke<boolean>('check_model_status'),
				invoke<boolean>('is_server_running'),
				invoke<boolean>('check_csm_status'),
				invoke<boolean>('is_csm_running')
			]);

			isGemmaDownloaded = gemmaDownloaded;
			isGemmaLoaded = gemmaLoaded;
			isCsmDownloaded = csmDownloaded;
			isCsmLoaded = csmLoaded;
		} catch (err) {
			console.error('Failed to sync model status:', err);
		}
	}

	async function startAudioCapture() {
		console.log('Starting audio capture...');

		try {
			mediaStream = await navigator.mediaDevices.getUserMedia({
				audio: {
					echoCancellation: true,
					noiseSuppression: true
				}
			});
			captureContext = new AudioContext();

			await captureContext.audioWorklet.addModule('/audio-processor.js');

			source = captureContext.createMediaStreamSource(mediaStream);
			processor = new AudioWorkletNode(captureContext, 'audio-processor');

			processor.port.onmessage = (event) => {
				if (micMuted || !calling) {
					return;
				}

				const inputData = event.data as Float32Array;
				void invoke('receive_audio_chunk', { payload: { data: Array.from(inputData) } }).catch((err) =>
					console.error('Invoke error:', err)
				);
			};

			source.connect(processor);
			processor.connect(captureContext.destination);

			if (captureContext.state === 'suspended') {
				await captureContext.resume();
			}
		} catch (err) {
			console.error('Failed to start audio capture:', err);
			calling = false;
		}
	}

	function stopAudioCapture() {
		if (processor) {
			processor.disconnect();
			processor = null;
		}
		if (source) {
			source.disconnect();
			source = null;
		}
		if (captureContext) {
			void captureContext.close();
			captureContext = null;
		}
		if (mediaStream) {
			mediaStream.getTracks().forEach((track) => track.stop());
			mediaStream = null;
		}
	}

	function stopPlayback(_closeContext = false) {
		if (playbackIdleTimeout) {
			clearTimeout(playbackIdleTimeout);
			playbackIdleTimeout = null;
		}
		if (activePlaybackAudio) {
			activePlaybackAudio.pause();
			activePlaybackAudio.src = '';
			activePlaybackAudio = null;
		}
		if (activePlaybackUrl) {
			URL.revokeObjectURL(activePlaybackUrl);
			activePlaybackUrl = null;
		}
		for (const url of queuedPlaybackUrls) {
			URL.revokeObjectURL(url);
		}
		queuedPlaybackUrls = [];
		activeTtsRequestId = null;
		pendingTtsSegments = 0;
	}

	function updateStageAfterPlaybackStateChange() {
		if (!calling) {
			return;
		}

		if (activePlaybackAudio || queuedPlaybackUrls.length > 0) {
			setCallStage('speaking', 'Speaking');
			return;
		}

		if (pendingTtsSegments > 0) {
			setCallStage('generating_audio', 'Generating Audio');
			return;
		}

		setCallStage('listening', 'Listening');
	}

	function decodeBase64Bytes(audioBase64: string) {
		const binary = atob(audioBase64);
		const bytes = new Uint8Array(binary.length);

		for (let i = 0; i < binary.length; i += 1) {
			bytes[i] = binary.charCodeAt(i);
		}

		return bytes;
	}

	function playNextQueuedAudio() {
		if (!calling || activePlaybackAudio || queuedPlaybackUrls.length === 0) {
			return;
		}

		const nextUrl = queuedPlaybackUrls.shift();
		if (!nextUrl) {
			return;
		}

		const audio = new Audio(nextUrl);
		activePlaybackAudio = audio;
		activePlaybackUrl = nextUrl;
		audio.preload = 'auto';
		if (playbackIdleTimeout) {
			clearTimeout(playbackIdleTimeout);
			playbackIdleTimeout = null;
		}
		updateStageAfterPlaybackStateChange();

		const finishPlayback = () => {
			if (activePlaybackAudio === audio) {
				activePlaybackAudio.pause();
				activePlaybackAudio.src = '';
				activePlaybackAudio = null;
			}
			if (activePlaybackUrl === nextUrl) {
				URL.revokeObjectURL(nextUrl);
				activePlaybackUrl = null;
			}
			if (queuedPlaybackUrls.length === 0 && calling) {
				playbackIdleTimeout = window.setTimeout(() => {
					if (!activePlaybackAudio && queuedPlaybackUrls.length === 0 && calling) {
						updateStageAfterPlaybackStateChange();
					}
					playbackIdleTimeout = null;
				}, 450);
			}
			playNextQueuedAudio();
		};

		audio.onended = finishPlayback;
		audio.onerror = () => {
			console.error('Failed to play queued CSM audio');
			finishPlayback();
		};

		void audio.play().catch((err) => {
			console.error('Failed to start queued CSM audio:', err);
			finishPlayback();
		});
	}

	async function queuePlaybackChunk(payload: CsmAudioChunkEvent) {
		if (!calling) {
			return;
		}
		if (activeTtsRequestId !== payload.request_id) {
			return;
		}

		const audioBytes = decodeBase64Bytes(payload.audio_wav_base64);
		if (audioBytes.length === 0) {
			return;
		}

		const audioBlob = new Blob([audioBytes], { type: 'audio/wav' });
		queuedPlaybackUrls = [...queuedPlaybackUrls, URL.createObjectURL(audioBlob)];
		playNextQueuedAudio();
	}

	async function handleStartCall() {
		if (!modelsReady) {
			return;
		}

		try {
			await applyCsmVoiceSelection();
		} catch (err) {
			console.error('Failed to apply CSM voice:', err);
			alert(`Failed to set CSM voice.\n${String(err)}`);
			return;
		}

		try {
			await invoke('reset_call_session');
		} catch (err) {
			console.error('Failed to reset call session:', err);
		}

		calling = true;
		time = 0;
		activeTtsRequestId = null;
		setCallStage('listening', 'Listening');

		void startAudioCapture();
		void invoke('ping').catch((err) => console.error('Backend ping failed', err));

		if (callTimerInterval) {
			clearInterval(callTimerInterval);
		}

		callTimerInterval = window.setInterval(() => {
			if (!calling) {
				if (callTimerInterval) {
					clearInterval(callTimerInterval);
					callTimerInterval = null;
				}
				return;
			}
			time += 1;
		}, 1000);
	}

	async function handleEndCall() {
		calling = false;
		stopAudioCapture();
		stopPlayback();
		setCallStage('idle', '');

		if (callTimerInterval) {
			clearInterval(callTimerInterval);
			callTimerInterval = null;
		}

		try {
			await invoke('reset_call_session');
		} catch (err) {
			console.error('Failed to clear call session:', err);
		}
	}

	function toggleMic() {
		micMuted = !micMuted;
	}

	async function handleDownloadGemma() {
		isDownloadingGemma = true;
		resetDownloadState('gemma');
		try {
			await invoke('download_model');
			await syncModelStatus();
		} catch (err) {
			console.error('Download model failed:', err);
			alert(`Gemma download failed.\n${String(err)}`);
		} finally {
			isDownloadingGemma = false;
		}
	}

	async function handleLoadGemma() {
		isLoadingGemma = true;
		try {
			await invoke('start_server');
			for (let i = 0; i < 30; i += 1) {
				await new Promise((resolve) => setTimeout(resolve, 1000));
				if (await invoke<boolean>('is_server_running')) {
					isGemmaLoaded = true;
					break;
				}
			}
		} catch (err) {
			console.error('Load model failed:', err);
			alert(`Failed to load Gemma.\n${String(err)}`);
		} finally {
			isLoadingGemma = false;
			await syncModelStatus();
		}
	}

	async function handleDownloadCsm() {
		isDownloadingCsm = true;
		resetDownloadState('csm');
		try {
			await invoke('download_csm_model');
			await syncModelStatus();
		} catch (err) {
			console.error('Download CSM failed:', err);
			alert(`CSM download failed.\n${String(err)}`);
		} finally {
			isDownloadingCsm = false;
		}
	}

	async function handleLoadCsm() {
		isLoadingCsm = true;
		csmLoadMessage = 'Starting worker...';
		try {
			await applyCsmVoiceSelection();
			await invoke('start_csm_server');
			isCsmLoaded = true;
		} catch (err) {
			console.error('Load CSM failed:', err);
			alert(`Failed to load CSM.\n${String(err)}`);
		} finally {
			isLoadingCsm = false;
			await syncModelStatus();
		}
	}

	onMount(() => {
		void syncModelStatus();

		healthCheckInterval = window.setInterval(() => {
			void syncModelStatus();
		}, 5000);

		void (async () => {
			try {
				eventUnlisteners = await Promise.all([
					listen<CsmAudioStartEvent>('csm-audio-start', ({ payload }) => {
						if (!calling) {
							return;
						}

						stopPlayback();
						activeTtsRequestId = payload.request_id;
						pendingTtsSegments = payload.total_segments;
						console.log('Synthesizing response:', payload.text);
					}),
					listen<CsmAudioChunkEvent>('csm-audio-chunk', ({ payload }) => {
						void queuePlaybackChunk(payload);
					}),
					listen<CsmAudioDoneEvent>('csm-audio-done', ({ payload }) => {
						if (payload.request_id === activeTtsRequestId) {
							pendingTtsSegments = Math.max(0, pendingTtsSegments - 1);
							if (!activePlaybackAudio && queuedPlaybackUrls.length === 0) {
								updateStageAfterPlaybackStateChange();
							}
							console.log('Finished streaming CSM response');
						}
					}),
					listen<CsmAudioStopEvent>('csm-audio-stop', () => {
						stopPlayback();
						if (calling) {
							updateStageAfterPlaybackStateChange();
						}
					}),
					listen<CsmErrorEvent>('csm-error', ({ payload }) => {
						console.error('CSM error:', payload.message);
						if (payload.request_id == null || payload.request_id === activeTtsRequestId) {
							stopPlayback();
							if (calling) {
								updateStageAfterPlaybackStateChange();
							}
						}
					}),
					listen<CsmStatusEvent>('csm-status', ({ payload }) => {
						if (isLoadingCsm) {
							csmLoadMessage = payload.message;
						}
					}),
					listen<CallStageEvent>('call-stage', ({ payload }) => {
						if (!calling) {
							return;
						}

						if (
							payload.phase === 'processing_audio' ||
							payload.phase === 'thinking' ||
							payload.phase === 'generating_audio'
						) {
							setCallStage(
								payload.phase,
								payload.message
							);
						}
					}),
					listen<ModelDownloadProgressEvent>('model-download-progress', ({ payload }) => {
						applyDownloadEvent(payload);
					})
				]);
			} catch (err) {
				console.error('Failed to register Tauri event listeners:', err);
			}
		})();
	});

	onDestroy(() => {
		if (healthCheckInterval) {
			clearInterval(healthCheckInterval);
		}
		if (callTimerInterval) {
			clearInterval(callTimerInterval);
		}
		if (playbackIdleTimeout) {
			clearTimeout(playbackIdleTimeout);
		}

		stopAudioCapture();
		stopPlayback(true);

		for (const unlisten of eventUnlisteners) {
			unlisten();
		}
		eventUnlisteners = [];
	});
</script>

<div class="app-container">
	<div class="background"></div>

	{#if !calling}
		<div class="model-tags">
			<div class="download-banner" class:ready={isGemmaDownloaded && isGemmaLoaded}>
				{#if isDownloadingGemma}
					<div class="download-content">
						<div class="download-row">
							<span>Gemma 3: {gemmaDownloadMessage}</span>
							{#if gemmaDownloadProgress !== null}
								<span class="download-percent">{Math.round(gemmaDownloadProgress)}%</span>
							{/if}
						</div>
						<div class="progress-track">
							<div
								class="progress-fill"
								class:indeterminate={gemmaDownloadIndeterminate}
								style:width={gemmaDownloadIndeterminate ? '38%' : `${gemmaDownloadProgress ?? 0}%`}
							></div>
						</div>
					</div>
				{:else if isGemmaDownloaded}
					<div class="banner-row">
						{#if isGemmaLoaded}
							<div class="banner-status">
								<div class="banner-copy">
									<span class="banner-title">Gemma 3</span>
									<span class="banner-subtitle">Loaded</span>
								</div>
								<div class="status-icon">
									<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#34c759" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
								</div>
							</div>
						{:else}
							<div class="banner-copy">
								<span class="banner-title">Gemma 3</span>
								<span class="banner-subtitle">Downloaded</span>
							</div>
							<button class="download-btn" disabled={isLoadingGemma} onclick={handleLoadGemma}>
								{isLoadingGemma ? 'Loading...' : 'Load Model'}
							</button>
						{/if}
					</div>
				{:else}
					<div class="banner-row">
						<div class="banner-copy">
							<span class="banner-title">Gemma 3</span>
							<span class="banner-subtitle">Model not found in cache</span>
						</div>
						<button class="download-btn" disabled={isDownloadingGemma} onclick={handleDownloadGemma}>
							{isDownloadingGemma ? 'Downloading...' : 'Download Model'}
						</button>
					</div>
				{/if}
			</div>

			<div class="download-banner voice-config-banner" class:ready={isCsmDownloaded && isCsmLoaded}>
				{#if isDownloadingCsm}
					<div class="download-content">
						<div class="download-row">
							<span>CSM 1B: {csmDownloadMessage}</span>
							{#if csmDownloadProgress !== null}
								<span class="download-percent">{Math.round(csmDownloadProgress)}%</span>
							{/if}
						</div>
						<div class="progress-track">
							<div
								class="progress-fill"
								class:indeterminate={csmDownloadIndeterminate}
								style:width={csmDownloadIndeterminate ? '38%' : `${csmDownloadProgress ?? 0}%`}
							></div>
						</div>
					</div>
				{:else if isCsmDownloaded}
					<div class="banner-row">
						{#if isCsmLoaded}
							<div class="banner-status">
								<div class="banner-copy">
									<span class="banner-title">CSM 1B</span>
									<span class="banner-subtitle">Loaded</span>
								</div>
								<div class="status-icon">
									<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#34c759" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
								</div>
							</div>
						{:else}
							<div class="banner-copy">
								<span class="banner-title">CSM 1B</span>
								<span class="banner-subtitle">{isLoadingCsm ? csmLoadMessage : 'Downloaded'}</span>
							</div>
							<button class="download-btn" disabled={isLoadingCsm} onclick={handleLoadCsm}>
								{isLoadingCsm ? 'Loading...' : 'Load Model'}
							</button>
						{/if}
					</div>
				{:else}
					<div class="banner-row">
						<div class="banner-copy">
							<span class="banner-title">CSM 1B</span>
							<span class="banner-subtitle">Model not found in cache</span>
						</div>
						<button class="download-btn" disabled={isDownloadingCsm} onclick={handleDownloadCsm}>
							{isDownloadingCsm ? 'Downloading...' : 'Download Model'}
						</button>
					</div>
				{/if}

				<div class="voice-select-row">
					<label class="voice-label" for="csm-voice-select">Voice</label>
					<div class="voice-select-shell">
						<select
							id="csm-voice-select"
							class="voice-select"
							bind:value={selectedCsmVoice}
							disabled={isLoadingCsm}
							onchange={handleCsmVoiceChange}
						>
							<option value="female">Female</option>
							<option value="male">Male</option>
						</select>
						<svg
							class="voice-select-chevron"
							xmlns="http://www.w3.org/2000/svg"
							width="16"
							height="16"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2.4"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path d="m6 9 6 6 6-6"></path>
						</svg>
					</div>
				</div>
			</div>
		</div>
	{/if}

	<main class:idle-layout={!calling}>
		{#if calling && callStageMessage}
			<div class="call-stage-banner" data-phase={callStagePhase}>
				<span class="call-stage-dot"></span>
				<span>{callStageMessage}</span>
			</div>
		{/if}
		<div class="avatar-container">
			<div class="avatar" class:calling></div>
		</div>
	</main>

	<div class="control-bar-wrapper">
		<div class="control-bar">
			<div class="info">
				<div class="username">openduck</div>
				<div class="timer">{calling ? formattedTime : 'Ready'}</div>
			</div>

			<div class="actions">
				{#if calling}
					<button class="icon-btn" class:active={!micMuted} onclick={toggleMic} aria-label={micMuted ? 'Unmute microphone' : 'Mute microphone'}>
						{#if micMuted}
							<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="1" y1="1" x2="23" y2="23"/><path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6"/><path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23"/><line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/></svg>
						{:else}
							<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/></svg>
						{/if}
					</button>
				{/if}
			</div>

			{#if calling}
				<button class="end-btn" onclick={handleEndCall}>End</button>
			{:else}
				<button class="start-btn" disabled={!modelsReady} onclick={handleStartCall}>Call</button>
			{/if}
		</div>
	</div>
</div>

<style>
	:global(body) {
		margin: 0;
		padding: 0;
		overflow: hidden;
		font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', Arial, sans-serif;
		height: 100vh;
		width: 100vw;
		background: #000;
	}

	.app-container {
		position: relative;
		width: 100%;
		height: 100vh;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.background {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background-image: url('/icon.png');
		background-size: cover;
		background-position: center;
		filter: blur(60px) brightness(0.6) saturate(1.2);
		transform: scale(1.2);
		z-index: -1;
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 28px;
		width: 100%;
		box-sizing: border-box;
		padding: 0 24px;
	}

	main.idle-layout {
		padding-top: clamp(144px, 24vh, 196px);
	}

	.call-stage-banner {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 16px;
		border-radius: 999px;
		background: rgba(28, 28, 30, 0.78);
		border: 1px solid rgba(255, 255, 255, 0.1);
		color: rgba(255, 255, 255, 0.92);
		backdrop-filter: blur(14px);
		font-size: 0.98rem;
		font-weight: 600;
		letter-spacing: -0.01em;
		box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
	}

	.call-stage-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: #7fe37c;
		box-shadow: 0 0 0 0 rgba(127, 227, 124, 0.5);
		animation: callPulse 1.4s ease-out infinite;
	}

	.call-stage-banner[data-phase='processing_audio'] .call-stage-dot {
		background: #ffd25f;
		box-shadow: 0 0 0 0 rgba(255, 210, 95, 0.5);
	}

	.call-stage-banner[data-phase='thinking'] .call-stage-dot {
		background: #7cc8ff;
		box-shadow: 0 0 0 0 rgba(124, 200, 255, 0.5);
	}

	.call-stage-banner[data-phase='generating_audio'] .call-stage-dot {
		background: #ff9f68;
		box-shadow: 0 0 0 0 rgba(255, 159, 104, 0.5);
	}

	.call-stage-banner[data-phase='speaking'] .call-stage-dot {
		background: #7fe37c;
		box-shadow: 0 0 0 0 rgba(127, 227, 124, 0.5);
	}

	.avatar {
		width: 140px;
		height: 140px;
		border-radius: 50%;
		background-image: url('/icon.png');
		background-size: cover;
		background-position: center;
		box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
		transition: box-shadow 0.5s ease-in-out;
	}

	.avatar.calling {
		box-shadow: 0 0 60px rgba(255, 215, 0, 0.4);
	}

	.model-tags {
		position: absolute;
		top: 40px;
		display: flex;
		flex-direction: column;
		align-items: center;
		width: min(calc(100vw - 48px), 560px);
		z-index: 10;
	}

	.control-bar-wrapper {
		position: absolute;
		bottom: 40px;
		width: 100%;
		display: flex;
		justify-content: center;
		padding: 0 20px;
	}

	.control-bar {
		background-color: #2c2c2e;
		border-radius: 32px;
		padding: 14px 28px;
		display: flex;
		align-items: center;
		gap: 36px;
		box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
		width: auto;
		min-width: 440px;
		border: 1px solid rgba(255, 255, 255, 0.05);
	}

	.info {
		display: flex;
		flex-direction: column;
		color: white;
		min-width: 140px;
	}

	.username {
		font-weight: 600;
		font-size: 1.15rem;
		letter-spacing: -0.01em;
	}

	.timer {
		font-size: 1rem;
		opacity: 0.6;
		margin-top: 2px;
	}

	.actions {
		display: flex;
		gap: 14px;
		flex: 1;
		justify-content: center;
	}

	.icon-btn {
		background: #444448;
		border: none;
		border-radius: 50%;
		width: 48px;
		height: 48px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: white;
		cursor: pointer;
		transition: background-color 0.2s, transform 0.1s;
	}

	.icon-btn:hover {
		background: #545458;
	}

	.icon-btn:active {
		transform: scale(0.95);
	}

	.icon-btn.active {
		background: #ffffff;
		color: #1c1c1e;
	}

	.end-btn,
	.start-btn {
		color: white;
		border: none;
		border-radius: 24px;
		padding: 10px 30px;
		font-weight: 600;
		font-size: 1.05rem;
		cursor: pointer;
		transition: background-color 0.2s, transform 0.1s, opacity 0.2s;
	}

	.end-btn {
		background-color: #ff3b30;
	}

	.end-btn:hover {
		background-color: #ff453a;
	}

	.start-btn {
		background-color: #34c759;
	}

	.start-btn:hover:not(:disabled) {
		background-color: #30d158;
	}

	.end-btn:active,
	.start-btn:active:not(:disabled) {
		transform: scale(0.95);
	}

	.start-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.download-banner {
		background: rgba(28, 28, 30, 0.9);
		border: 1px solid rgba(255, 255, 255, 0.1);
		border-radius: 16px;
		padding: 14px 24px;
		display: flex;
		align-items: center;
		gap: 16px;
		color: white;
		backdrop-filter: blur(10px);
		animation: slideDown 0.3s ease-out;
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
	}

	.download-banner.ready {
		background: rgba(28, 28, 30, 0.6);
		padding: 10px 20px;
	}

	.download-banner.voice-config-banner {
		align-items: stretch;
		flex-direction: column;
		gap: 14px;
	}

	.download-banner.voice-config-banner.ready {
		padding: 14px 24px;
	}

	.download-content {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
	}

	.download-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
	}

	.banner-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		width: 100%;
	}

	.banner-copy {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex: 1;
		min-width: 0;
	}

	.banner-title {
		font-size: 1rem;
		font-weight: 600;
		letter-spacing: -0.015em;
	}

	.banner-subtitle {
		color: rgba(255, 255, 255, 0.62);
		font-size: 0.92rem;
		letter-spacing: -0.01em;
	}

	.banner-status {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		width: 100%;
	}

	.download-percent {
		color: rgba(255, 255, 255, 0.75);
		font-variant-numeric: tabular-nums;
	}

	.progress-track {
		position: relative;
		width: 100%;
		height: 8px;
		border-radius: 999px;
		background: rgba(255, 255, 255, 0.1);
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		border-radius: 999px;
		background: linear-gradient(90deg, #7fe37c 0%, #34c759 100%);
		transition: width 0.2s ease;
	}

	.progress-fill.indeterminate {
		position: relative;
		animation: indeterminateSlide 1.1s ease-in-out infinite;
	}

	.download-btn {
		background: #ffffff;
		color: #000;
		border: none;
		border-radius: 8px;
		padding: 6px 14px;
		font-weight: 600;
		cursor: pointer;
		font-size: 0.9rem;
	}

	.download-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.status-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border-radius: 999px;
		background: rgba(52, 199, 89, 0.12);
		flex-shrink: 0;
	}

	.status-icon :global(svg) {
		display: block;
	}

	.voice-select-row {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 10px;
		padding-top: 14px;
		border-top: 1px solid rgba(255, 255, 255, 0.08);
	}

	.voice-label {
		color: rgba(255, 255, 255, 0.58);
		font-size: 0.78rem;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.voice-select-shell {
		position: relative;
		width: 100%;
		min-width: 0;
	}

	.voice-select {
		appearance: none;
		width: 100%;
		border: 1px solid rgba(255, 255, 255, 0.08);
		border-radius: 14px;
		background: rgba(255, 255, 255, 0.07);
		color: white;
		padding: 13px 42px 13px 16px;
		font-size: 1rem;
		font-weight: 600;
		letter-spacing: -0.01em;
		outline: none;
		cursor: pointer;
		color-scheme: dark;
		transition: border-color 0.2s ease, box-shadow 0.2s ease, background-color 0.2s ease;
	}

	.voice-select:focus {
		border-color: rgba(127, 227, 124, 0.45);
		box-shadow: 0 0 0 3px rgba(127, 227, 124, 0.12);
	}

	.voice-select:hover:not(:disabled) {
		background: rgba(255, 255, 255, 0.1);
	}

	.voice-select:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}

	.voice-select-chevron {
		position: absolute;
		top: 50%;
		right: 12px;
		transform: translateY(-50%);
		color: rgba(255, 255, 255, 0.62);
		pointer-events: none;
	}

	@keyframes slideDown {
		from {
			transform: translateY(-20px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@keyframes indeterminateSlide {
		from {
			transform: translateX(-120%);
		}
		to {
			transform: translateX(320%);
		}
	}

	@keyframes callPulse {
		0% {
			transform: scale(1);
			box-shadow: 0 0 0 0 currentColor;
		}
		70% {
			transform: scale(1.08);
			box-shadow: 0 0 0 10px rgba(255, 255, 255, 0);
		}
		100% {
			transform: scale(1);
			box-shadow: 0 0 0 0 rgba(255, 255, 255, 0);
		}
	}
</style>
