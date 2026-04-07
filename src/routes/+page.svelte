<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';

	let calling = $state(false);
	let micMuted = $state(false);
	let time = $state(0); 
	let isModelDownloaded = $state(false);
	let isModelLoaded = $state(false);
	let isDownloading = $state(false);
	let isLoadingModel = $state(false);

	let audioContext: AudioContext | null = null;
	let mediaStream: MediaStream | null = null;
	let source: MediaStreamAudioSourceNode | null = null;
	let processor: AudioWorkletNode | null = null;
	let healthCheckInterval: any = null;

	async function startAudioCapture() {
		console.log('Starting audio capture...');
		try {
			mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
			console.log('Microphone access granted');
			
			audioContext = new AudioContext();
			console.log('AudioContext created, state:', audioContext.state, 'sampleRate:', audioContext.sampleRate);
			
			// Load and add the AudioWorklet module
			const moduleUrl = '/audio-processor.js';
			console.log('Loading AudioWorklet from:', moduleUrl);
			await audioContext.audioWorklet.addModule(moduleUrl);
			console.log('AudioWorklet module added');
			
			source = audioContext.createMediaStreamSource(mediaStream);
			processor = new AudioWorkletNode(audioContext, 'audio-processor');
			
			processor.port.onmessage = (event) => {
				if (micMuted || !calling) return;
				
				const inputData = event.data;
				// Send to Rust backend
				invoke('receive_audio_chunk', { payload: { data: Array.from(inputData) } })
					.catch(err => console.error('Invoke error:', err));
			};

			source.connect(processor);
			processor.connect(audioContext.destination);
			
			if (audioContext.state === 'suspended') {
				await audioContext.resume();
				console.log('AudioContext resumed');
			}
			
			console.log('Audio pipeline connected');
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
		if (audioContext) {
			audioContext.close();
			audioContext = null;
		}
		if (mediaStream) {
			mediaStream.getTracks().forEach(track => track.stop());
			mediaStream = null;
		}
	}

	function handleStartCall() {
		console.log('Button: Call clicked');
		calling = true;
		time = 0;
		startAudioCapture();
		
		// Ping backend to confirm connection
		invoke('ping').then(() => console.log('Backend: ping successful')).catch(err => console.error('Backend: ping failed', err));
		
		// Start timer simulation
		const interval = setInterval(() => {
			if (!calling) {
				clearInterval(interval);
				return;
			}
			time++;
		}, 1000);
	}

	function handleEndCall() {
		calling = false;
		stopAudioCapture();
	}

	function toggleMic() {
		micMuted = !micMuted;
	}

	// Format time as mm:ss
	const formattedTime = $derived(`${Math.floor(time / 60).toString().padStart(2, '0')}:${(time % 60).toString().padStart(2, '0')}`);

	onMount(() => {
		const checkStatus = async () => {
			try {
				isModelDownloaded = await invoke('check_model_status');
				isModelLoaded = await invoke('is_server_running');
			} catch (err) {
				console.error('Failed to check status:', err);
			}
		};
		
		checkStatus();

		// Periodic health check
		healthCheckInterval = setInterval(async () => {
			isModelLoaded = await invoke('is_server_running');
		}, 5000);
	});

	onDestroy(() => {
		if (healthCheckInterval) clearInterval(healthCheckInterval);
	});

	async function handleDownloadModel() {
		isDownloading = true;
		try {
			await invoke('download_model');
			isModelDownloaded = await invoke('check_model_status');
		} catch (err) {
			console.error('Download model failed:', err);
			alert('Download failed. Check the console for details.');
		} finally {
			isDownloading = false;
		}
	}

	async function handleLoadModel() {
		isLoadingModel = true;
		try {
			await invoke('start_server');
			// Wait for server to be responsive
			for (let i = 0; i < 30; i++) {
				await new Promise(r => setTimeout(r, 1000));
				if (await invoke('is_server_running')) {
					isModelLoaded = true;
					break;
				}
			}
		} catch (err) {
			console.error('Load model failed:', err);
			alert('Failed to load model. Check logs.');
		} finally {
			isLoadingModel = false;
		}
	}
</script>

<div class="app-container">
	<!-- Background -->
	<div class="background"></div>

	<!-- Main Content -->
	<main>
		<div class="avatar-container">
			<div class="avatar" class:calling></div>
		</div>
	</main>

	{#if !calling}
		<div class="download-banner" class:ready={isModelDownloaded && isModelLoaded}>
			{#if isModelDownloaded}
				{#if isModelLoaded}
					<div class="status-icon">
						<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#34c759" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
					</div>
					<span>Gemma 3: Loaded</span>
				{:else}
					<span>Gemma 3: Downloaded</span>
					<button class="download-btn" disabled={isLoadingModel} onclick={handleLoadModel}>
						{isLoadingModel ? "Loading..." : "Load Model"}
					</button>
				{/if}
			{:else}
				<span>Gemma 3 model not found in cache.</span>
				<button class="download-btn" disabled={isDownloading} onclick={handleDownloadModel}>
					{isDownloading ? "Downloading..." : "Download Model"}
				</button>
			{/if}
		</div>
	{/if}

	<!-- Control Bar -->
	<div class="control-bar-wrapper">
		<div class="control-bar">
			<div class="info">
				<div class="username">openduck</div>
				<div class="timer">{calling ? formattedTime : "Ready"}</div>
			</div>

			<div class="actions">
				{#if calling}
					<button class="icon-btn" class:active={!micMuted} onclick={toggleMic} aria-label={micMuted ? "Unmute microphone" : "Mute microphone"}>
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
				<button class="start-btn" onclick={handleStartCall}>Call</button>
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
		align-items: center;
		justify-content: center;
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

	.end-btn {
		background-color: #ff3b30;
		color: white;
		border: none;
		border-radius: 24px;
		padding: 10px 30px;
		font-weight: 600;
		font-size: 1.05rem;
		cursor: pointer;
		transition: background-color 0.2s, transform 0.1s;
	}

	.end-btn:hover {
		background-color: #ff453a;
	}

	.end-btn:active {
		transform: scale(0.95);
	}

	.start-btn {
		background-color: #34c759;
		color: white;
		border: none;
		border-radius: 24px;
		padding: 10px 30px;
		font-weight: 600;
		font-size: 1.05rem;
		cursor: pointer;
		transition: background-color 0.2s, transform 0.1s;
	}

	.start-btn:hover {
		background-color: #30d158;
	}

	.start-btn:active {
		transform: scale(0.95);
	}

	.download-banner {
		position: absolute;
		top: 40px;
		background: rgba(28, 28, 30, 0.9);
		border: 1px solid rgba(255, 255, 255, 0.1);
		border-radius: 12px;
		padding: 12px 20px;
		display: flex;
		align-items: center;
		gap: 16px;
		color: white;
		backdrop-filter: blur(10px);
		z-index: 10;
		animation: slideDown 0.3s ease-out;
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

	.download-banner.ready {
		background: rgba(28, 28, 30, 0.6);
		padding: 8px 16px;
	}

	.status-icon {
		display: flex;
		align-items: center;
		justify-content: center;
	}

	@keyframes slideDown {
		from { transform: translateY(-20px); opacity: 0; }
		to { transform: translateY(0); opacity: 1; }
	}
</style>
