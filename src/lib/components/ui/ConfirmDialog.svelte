<script lang="ts">
    let { title, message, onConfirm, onCancel } = $props<{
        title: string;
        message: string;
        onConfirm: () => void;
        onCancel: () => void;
    }>();
</script>

<div class="confirm-overlay" role="presentation" onclick={onCancel}>
    <div class="confirm-dialog" role="dialog" aria-modal="true" onclick={(e) => e.stopPropagation()}>
        <h2 class="confirm-title">{title}</h2>
        <p class="confirm-message">{message}</p>
        <div class="confirm-actions">
            <button type="button" class="confirm-btn cancel" onclick={onCancel}>
                Cancel
            </button>
            <button type="button" class="confirm-btn confirm" onclick={onConfirm}>
                Confirm
            </button>
        </div>
    </div>
</div>

<style>
    .confirm-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.4);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10000;
        backdrop-filter: blur(4px);
    }

    .confirm-dialog {
        background: white;
        border-radius: 28px;
        padding: 32px;
        width: 100%;
        max-width: 400px;
        box-shadow: 0 20px 40px rgba(0, 0, 0, 0.2);
        display: flex;
        flex-direction: column;
        gap: 16px;
    }

    .confirm-title {
        margin: 0;
        font-size: 1.5rem;
        font-weight: 700;
        color: #1c1c1e;
        letter-spacing: -0.02em;
    }

    .confirm-message {
        margin: 0;
        font-size: 1.05rem;
        line-height: 1.5;
        color: #48484a;
    }

    .confirm-actions {
        display: flex;
        gap: 12px;
        margin-top: 12px;
    }

    .confirm-btn {
        flex: 1;
        border: none;
        border-radius: 999px;
        padding: 14px;
        font-size: 1rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s;
    }

    .confirm-btn.cancel {
        background: #f2f2f7;
        color: #1c1c1e;
    }

    .confirm-btn.confirm {
        background: #1c1c1e;
        color: white;
    }

    .confirm-btn:hover {
        transform: scale(0.98);
        opacity: 0.9;
    }

    .confirm-btn:active {
        transform: scale(0.96);
    }
</style>
