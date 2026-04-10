import argparse
import inspect
import json
import os
import re
import shutil
import sys
import traceback
from pathlib import Path

from huggingface_hub import constants, hf_hub_download, snapshot_download
from huggingface_hub.file_download import (
    _get_metadata_or_catch_error,
    repo_folder_name,
)
from tqdm.auto import tqdm

DEVNULL = open(os.devnull, "w")


def emit(payload: dict) -> None:
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


class SingleFileProgressTqdm(tqdm):
    model = ""
    message = "Downloading model files..."

    def __init__(self, *args, **kwargs):
        kwargs.setdefault("file", DEVNULL)
        kwargs.setdefault("leave", False)
        kwargs.setdefault("mininterval", 0.1)
        self._last_signature: tuple[int, int | None] | None = None
        super().__init__(*args, **kwargs)
        self._emit_progress(force=True)

    def update(self, n=1):
        result = super().update(n)
        self._emit_progress()
        return result

    def refresh(self, *args, **kwargs):
        result = super().refresh(*args, **kwargs)
        self._emit_progress()
        return result

    def close(self):
        self._emit_progress(force=True)
        return super().close()

    def _emit_progress(self, force: bool = False) -> None:
        total_bytes = int(self.total) if self.total else None
        downloaded_bytes = int(self.n)
        signature = (downloaded_bytes, total_bytes)

        if not force and signature == self._last_signature:
            return

        self._last_signature = signature
        progress = None
        if total_bytes:
            progress = max(
                0.0,
                min(100.0, float(downloaded_bytes) / float(total_bytes) * 100.0),
            )

        emit(
            {
                "type": "progress",
                "model": self.model,
                "message": self.message,
                "progress": progress,
                "downloaded_bytes": downloaded_bytes,
                "total_bytes": total_bytes,
                "indeterminate": progress is None and not force,
            }
        )


def split_filename(filename: str) -> tuple[str | None, str]:
    path = Path(filename)
    subfolder = str(path.parent) if str(path.parent) != "." else None
    return subfolder, path.name


def build_download_manifest(
    repo_id: str,
    model: str,
    allow_patterns: list[str] | None,
) -> tuple[list[object], dict]:
    dry_run_items = snapshot_download(
        repo_id,
        allow_patterns=allow_patterns,
        dry_run=True,
    )
    storage_folder = Path(constants.HF_HUB_CACHE).joinpath(
        repo_folder_name(repo_id=repo_id, repo_type="model")
    )
    total_bytes = 0
    known_commit_hash = next(
        (
            str(commit_hash)
            for item in dry_run_items
            if (commit_hash := getattr(item, "commit_hash", None))
        ),
        None,
    )
    manifest_files = []

    for item in dry_run_items:
        file_size = int(getattr(item, "file_size", 0) or 0)
        filename = str(getattr(item, "filename", ""))
        commit_hash = getattr(item, "commit_hash", None)
        blob_path = None
        incomplete_path = None

        if commit_hash and filename:
            headers: dict[str, str] = {}
            (_, etag, _, expected_size, _, metadata_error) = _get_metadata_or_catch_error(
                repo_id=repo_id,
                filename=filename,
                repo_type="model",
                revision=str(commit_hash),
                endpoint=None,
                etag_timeout=constants.HF_HUB_ETAG_TIMEOUT,
                headers=headers,
                token=None,
                local_files_only=False,
            )
            if metadata_error is None and etag:
                blob_path = str(storage_folder.joinpath("blobs", etag))
                incomplete_path = f"{blob_path}.incomplete"
                if expected_size is not None:
                    file_size = int(expected_size)

        total_bytes += file_size
        manifest_files.append(
            {
                "filename": filename,
                "file_size": file_size,
                "local_path": str(getattr(item, "local_path", "")),
                "blob_path": blob_path,
                "incomplete_path": incomplete_path,
                "is_cached": bool(getattr(item, "is_cached", False)),
                "will_download": bool(getattr(item, "will_download", True)),
            }
        )

    manifest = {
        "type": "manifest",
        "model": model,
        "commit_hash": known_commit_hash,
        "total_bytes": total_bytes,
        "files": manifest_files,
    }
    return dry_run_items, manifest


def download_with_progress(repo_id: str, model: str, allow_patterns: list[str] | None) -> str:
    dry_run_items, manifest = build_download_manifest(repo_id, model, allow_patterns)
    emit(manifest)
    files_to_download = [item for item in dry_run_items if getattr(item, "will_download", True)]
    total_bytes = int(manifest.get("total_bytes") or 0)
    known_commit_hash = manifest.get("commit_hash")
    completed_bytes = sum(
        int(file_info.get("file_size", 0) or 0)
        for file_info in manifest.get("files", [])
        if not file_info.get("will_download", True)
    )

    if not files_to_download:
        emit(
            {
                "type": "progress",
                "model": model,
                "message": "Already cached.",
                "progress": 100.0,
                "downloaded_bytes": total_bytes or None,
                "total_bytes": total_bytes or None,
                "indeterminate": False,
            }
        )
        if known_commit_hash:
            storage_folder = Path(constants.HF_HUB_CACHE).joinpath(
                repo_folder_name(repo_id=repo_id, repo_type="model")
            )
            return str(storage_folder.joinpath("snapshots", known_commit_hash))
        return snapshot_download(
            repo_id,
            allow_patterns=allow_patterns,
            local_files_only=True,
        )

    if completed_bytes > 0 and total_bytes > 0:
        emit(
            {
                "type": "progress",
                "model": model,
                "message": "Found cached files. Resuming remaining downloads...",
                "progress": max(
                    0.0,
                    min(100.0, float(completed_bytes) / float(total_bytes) * 100.0),
                ),
                "downloaded_bytes": completed_bytes,
                "total_bytes": total_bytes,
                "indeterminate": False,
            }
        )

    class GlobalProgressTqdm(tqdm):
        current_file = ""
        current_offset = 0
        total_for_all_files = total_bytes
        model_name = model

        def __init__(self, *args, **kwargs):
            kwargs.setdefault("file", DEVNULL)
            kwargs.setdefault("leave", False)
            kwargs.setdefault("mininterval", 0.1)
            self._last_signature: tuple[str, int, int | None] | None = None
            super().__init__(*args, **kwargs)
            self._emit_progress(force=True)

        def update(self, n=1):
            result = super().update(n)
            self._emit_progress()
            return result

        def close(self):
            self._emit_progress(force=True)
            return super().close()

        def _emit_progress(self, force: bool = False) -> None:
            total_for_all_files = (
                int(self.total_for_all_files) if self.total_for_all_files > 0 else None
            )
            downloaded_bytes = self.current_offset + int(self.n)
            signature = (self.current_file, downloaded_bytes, total_for_all_files)

            if not force and signature == self._last_signature:
                return

            self._last_signature = signature
            progress = None
            if total_for_all_files:
                progress = max(
                    0.0,
                    min(
                        100.0,
                        float(downloaded_bytes)
                        / float(total_for_all_files)
                        * 100.0,
                    ),
                )

            emit(
                {
                    "type": "progress",
                    "model": self.model_name,
                    "message": f"Downloading {self.current_file}...",
                    "progress": progress,
                    "downloaded_bytes": downloaded_bytes,
                    "total_bytes": total_for_all_files,
                    "indeterminate": progress is None and not force,
                }
            )

    for item in files_to_download:
        subfolder, filename = split_filename(item.filename)
        GlobalProgressTqdm.current_file = item.filename
        GlobalProgressTqdm.current_offset = completed_bytes
        hf_hub_download(
            repo_id,
            filename=filename,
            subfolder=subfolder,
            revision=getattr(item, "commit_hash", None),
            tqdm_class=GlobalProgressTqdm,
        )
        completed_bytes += int(getattr(item, "file_size", 0) or 0)
        progress = None
        if total_bytes > 0:
            progress = max(0.0, min(100.0, float(completed_bytes) / float(total_bytes) * 100.0))
        emit(
            {
                "type": "progress",
                "model": model,
                "message": f"Downloaded {item.filename}",
                "progress": progress,
                "downloaded_bytes": completed_bytes,
                "total_bytes": total_bytes or None,
                "indeterminate": progress is None,
            }
        )

    if known_commit_hash:
        storage_folder = Path(constants.HF_HUB_CACHE).joinpath(
            repo_folder_name(repo_id=repo_id, repo_type="model")
        )
        return str(storage_folder.joinpath("snapshots", known_commit_hash))

    return snapshot_download(
        repo_id,
        allow_patterns=allow_patterns,
        local_files_only=True,
    )


def download_with_snapshot_progress(repo_id: str, model: str, allow_patterns: list[str] | None) -> str:
    class SnapshotProgressTqdm(SingleFileProgressTqdm):
        pass

    SnapshotProgressTqdm.model = model
    SnapshotProgressTqdm.message = "Downloading model files..."

    return snapshot_download(
        repo_id,
        allow_patterns=allow_patterns,
        tqdm_class=SnapshotProgressTqdm,
    )


def gemma_snapshot_is_complete(snapshot_path: str | Path) -> bool:
    snapshot_dir = Path(snapshot_path)

    if not snapshot_dir.joinpath("config.json").exists():
        return False
    if not (
        snapshot_dir.joinpath("tokenizer.model").exists()
        or snapshot_dir.joinpath("tokenizer.json").exists()
    ):
        return False
    if snapshot_dir.joinpath("model.safetensors").exists():
        return True
    if not snapshot_dir.joinpath("model.safetensors.index.json").exists():
        return False

    shard_pattern = re.compile(r"^model-(\d{5})-of-(\d{5})\.safetensors$")
    shard_numbers_by_total: dict[int, set[int]] = {}

    for entry in snapshot_dir.iterdir():
        match = shard_pattern.match(entry.name)
        if not match:
            continue

        shard_number = int(match.group(1))
        shard_total = int(match.group(2))
        shard_numbers_by_total.setdefault(shard_total, set()).add(shard_number)

    if len(shard_numbers_by_total) != 1:
        return False

    shard_total, shard_numbers = next(iter(shard_numbers_by_total.items()))
    expected_numbers = set(range(1, shard_total + 1))
    return shard_numbers == expected_numbers


def repair_incomplete_gemma_snapshot(
    repo_id: str,
    model: str,
    allow_patterns: list[str] | None,
    snapshot_path: str,
) -> str:
    if model != "gemma" or gemma_snapshot_is_complete(snapshot_path):
        return snapshot_path

    snapshot_dir = Path(snapshot_path)
    repo_cache_dir = snapshot_dir.parent.parent

    emit(
        {
            "type": "progress",
            "model": model,
            "message": "Detected stale cache. Retrying clean download...",
            "progress": None,
            "downloaded_bytes": None,
            "total_bytes": None,
            "indeterminate": True,
        }
    )

    shutil.rmtree(repo_cache_dir, ignore_errors=True)

    repaired_path = download_with_progress(repo_id, model, allow_patterns)
    if not gemma_snapshot_is_complete(repaired_path):
        raise RuntimeError("Gemma cache is still incomplete after retry")

    return repaired_path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-id", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--allow-pattern", action="append", default=[])
    args = parser.parse_args()

    allow_patterns = args.allow_pattern or None

    emit(
        {
            "type": "progress",
            "model": args.model,
            "message": "Preparing download...",
            "progress": 0.0,
            "downloaded_bytes": 0,
            "total_bytes": None,
            "indeterminate": True,
        }
    )

    try:
        supports_dry_run = "dry_run" in inspect.signature(snapshot_download).parameters
        supports_hf_tqdm = "tqdm_class" in inspect.signature(hf_hub_download).parameters

        if supports_dry_run and supports_hf_tqdm:
            path = download_with_progress(args.repo_id, args.model, allow_patterns)
        else:
            path = download_with_snapshot_progress(
                args.repo_id,
                args.model,
                allow_patterns,
            )

        path = repair_incomplete_gemma_snapshot(
            args.repo_id,
            args.model,
            allow_patterns,
            path,
        )

        emit(
            {
                "type": "completed",
                "model": args.model,
                "message": "Download complete.",
                "progress": 100.0,
                "downloaded_bytes": None,
                "total_bytes": None,
                "indeterminate": False,
                "path": path,
            }
        )
        return 0
    except Exception as exc:
        emit(
            {
                "type": "error",
                "model": args.model,
                "message": f"Download failed: {exc}",
                "progress": None,
                "downloaded_bytes": None,
                "total_bytes": None,
                "indeterminate": True,
            }
        )
        traceback.print_exc(file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
