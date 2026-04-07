import argparse
import inspect
import json
import os
import sys
import traceback
from pathlib import Path

from huggingface_hub import hf_hub_download, snapshot_download
from tqdm.auto import tqdm

DEVNULL = open(os.devnull, "w")


def emit(payload: dict) -> None:
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


class SingleFileProgressTqdm(tqdm):
    model = ""
    message = ""

    def __init__(self, *args, **kwargs):
        kwargs.setdefault("file", DEVNULL)
        kwargs.setdefault("leave", False)
        kwargs.setdefault("mininterval", 0.1)
        super().__init__(*args, **kwargs)
        self._emit_progress()

    def update(self, n=1):
        result = super().update(n)
        self._emit_progress()
        return result

    def close(self):
        self._emit_progress(force=True)
        return super().close()

    def _emit_progress(self, force: bool = False) -> None:
        progress = None
        if self.total:
            progress = max(0.0, min(100.0, float(self.n) / float(self.total) * 100.0))
        emit(
            {
                "type": "progress",
                "model": self.model,
                "message": self.message,
                "progress": progress,
                "indeterminate": progress is None and not force,
            }
        )


def split_filename(filename: str) -> tuple[str | None, str]:
    path = Path(filename)
    subfolder = str(path.parent) if str(path.parent) != "." else None
    return subfolder, path.name


def download_with_global_progress(repo_id: str, model: str, allow_patterns: list[str] | None) -> str:
    dry_run_items = snapshot_download(
        repo_id,
        allow_patterns=allow_patterns,
        dry_run=True,
    )

    files_to_download = [item for item in dry_run_items if getattr(item, "will_download", True)]
    total_bytes = sum(getattr(item, "file_size", 0) or 0 for item in files_to_download)

    if not files_to_download:
        emit(
            {
                "type": "progress",
                "model": model,
                "message": "Already cached.",
                "progress": 100.0,
                "indeterminate": False,
            }
        )
        return str(Path(dry_run_items[0].local_path).parent.parent) if dry_run_items else ""

    completed_bytes = 0

    class GlobalProgressTqdm(tqdm):
        current_file = ""
        current_offset = 0
        total_for_all_files = total_bytes
        model_name = model

        def __init__(self, *args, **kwargs):
            kwargs.setdefault("file", DEVNULL)
            kwargs.setdefault("leave", False)
            kwargs.setdefault("mininterval", 0.1)
            super().__init__(*args, **kwargs)
            self._emit_progress()

        def update(self, n=1):
            result = super().update(n)
            self._emit_progress()
            return result

        def close(self):
            self._emit_progress(force=True)
            return super().close()

        def _emit_progress(self, force: bool = False) -> None:
            if self.total_for_all_files <= 0:
                progress = None
            else:
                progress = max(
                    0.0,
                    min(
                        100.0,
                        float(self.current_offset + self.n) / float(self.total_for_all_files) * 100.0,
                    ),
                )

            emit(
                {
                    "type": "progress",
                    "model": self.model_name,
                    "message": f"Downloading {self.current_file}...",
                    "progress": progress,
                    "indeterminate": progress is None and not force,
                }
            )

    last_path = ""
    for item in files_to_download:
        subfolder, filename = split_filename(item.filename)
        GlobalProgressTqdm.current_file = item.filename
        GlobalProgressTqdm.current_offset = completed_bytes
        last_path = hf_hub_download(
            repo_id,
            filename=filename,
            subfolder=subfolder,
            revision=getattr(item, "commit_hash", None),
            tqdm_class=GlobalProgressTqdm,
        )
        completed_bytes += getattr(item, "file_size", 0) or 0
        emit(
            {
                "type": "progress",
                "model": model,
                "message": f"Downloaded {item.filename}",
                "progress": max(0.0, min(100.0, float(completed_bytes) / float(total_bytes) * 100.0)),
                "indeterminate": False,
            }
        )

    return last_path


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
            "indeterminate": True,
        }
    )

    try:
        supports_dry_run = "dry_run" in inspect.signature(snapshot_download).parameters
        supports_hf_tqdm = "tqdm_class" in inspect.signature(hf_hub_download).parameters

        if supports_dry_run and supports_hf_tqdm:
            path = download_with_global_progress(args.repo_id, args.model, allow_patterns)
        else:
            path = download_with_snapshot_progress(args.repo_id, args.model, allow_patterns)

        emit(
            {
                "type": "completed",
                "model": args.model,
                "message": "Download complete.",
                "progress": 100.0,
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
                "indeterminate": True,
            }
        )
        traceback.print_exc(file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
