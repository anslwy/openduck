<script lang="ts">
	let calling = $state(false);
	let micMuted = $state(false);
	let time = $state(0); 

	function handleStartCall() {
		calling = true;
		time = 0;
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
	}

	function toggleMic() {
		micMuted = !micMuted;
	}

	// Format time as mm:ss
	const formattedTime = $derived(`${Math.floor(time / 60).toString().padStart(2, '0')}:${(time % 60).toString().padStart(2, '0')}`);
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
</style>
