<script lang="ts">
	let calling = $state(false);

	function handleCall() {
		calling = !calling;
		console.log(calling ? 'Starting call...' : 'Call ended');
	}
</script>

<main>
	<div class="container">
		<h1>OpenDuck</h1>
		
		<div class="phone-button" class:calling onclick={handleCall} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && handleCall()}>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="48" height="48" aria-hidden="true">
				<path d="M20 15.5c-1.25 0-2.45-.2-3.57-.57a1.02 1.02 0 00-1.02.24l-2.2 2.2a15.09 15.09 0 01-6.59-6.59l2.2-2.21a.96.96 0 00.25-1A11.36 11.36 0 018.5 4c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1 0 9.39 7.61 17 17 17 .55 0 1-.45 1-1v-3.5c0-.55-.45-1-1-1z"/>
			</svg>
		</div>
		
		<p class="status">{calling ? 'Calling...' : 'Tap to call'}</p>
	</div>
</main>

<style>
	:global(body) {
		margin: 0;
		font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
		background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
		color: #1a1a2e;
		min-height: 100vh;
	}

	main {
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 100vh;
	}

	.container {
		text-align: center;
	}

	h1 {
		font-size: 2.5rem;
		margin: 0;
		font-weight: 700;
	}

	.subtitle {
		color: #6b7280;
		margin-top: 0.5rem;
		font-size: 1.1rem;
	}

	.phone-button {
		width: 100px;
		height: 100px;
		border-radius: 50%;
		background: #fbbf24;
		display: flex;
		align-items: center;
		justify-content: center;
		margin: 3rem auto;
		cursor: pointer;
		transition: all 0.3s ease;
		box-shadow: 0 4px 20px rgba(251, 191, 36, 0.4);
	}

	.phone-button:hover {
		transform: scale(1.1);
		box-shadow: 0 6px 30px rgba(251, 191, 36, 0.6);
	}

	.phone-button.calling {
		background: #f59e0b;
		animation: pulse 1s infinite;
	}

	@keyframes pulse {
		0%, 100% { box-shadow: 0 0 0 0 rgba(245, 158, 11, 0.7); }
		50% { box-shadow: 0 0 0 20px rgba(245, 158, 11, 0); }
	}

	.status {
		color: #6b7280;
		font-size: 1rem;
	}
</style>