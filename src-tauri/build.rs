use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_LABEL");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_NUMBER");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_ID");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_GIT_SHA");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_GIT_DIRTY");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest_dir.clone());

    register_git_rerun_paths(&repo_root);

    let version =
        read_env_override("OPEN_DUCK_BUILD_VERSION").unwrap_or_else(|| cargo_package_version());
    let build_channel = read_env_override("OPEN_DUCK_BUILD_CHANNEL");
    let build_number = read_env_override("OPEN_DUCK_BUILD_NUMBER");
    let version_label = read_env_override("OPEN_DUCK_BUILD_LABEL")
        .or_else(|| default_label_for_channel(build_channel.as_deref()));
    let git_sha =
        read_env_override("OPEN_DUCK_GIT_SHA").or_else(|| git_output(&repo_root, &["rev-parse", "HEAD"]));
    let is_dirty = read_env_override("OPEN_DUCK_GIT_DIRTY")
        .map(|value| parse_truthy_flag(&value))
        .unwrap_or_else(|| git_is_dirty(&repo_root));
    let build_id = read_env_override("OPEN_DUCK_BUILD_ID").or_else(|| {
        compose_build_id(
            &version,
            version_label.as_deref(),
            build_channel.as_deref(),
            build_number.as_deref(),
            git_sha.as_deref(),
            is_dirty,
        )
    });

    emit_build_env("OPEN_DUCK_BUILD_VERSION", Some(&version));
    emit_build_env("OPEN_DUCK_BUILD_LABEL", version_label.as_deref());
    emit_build_env("OPEN_DUCK_BUILD_CHANNEL", build_channel.as_deref());
    emit_build_env("OPEN_DUCK_BUILD_NUMBER", build_number.as_deref());
    emit_build_env("OPEN_DUCK_BUILD_ID", build_id.as_deref());
    emit_build_env("OPEN_DUCK_GIT_SHA", git_sha.as_deref());
    emit_build_env("OPEN_DUCK_GIT_DIRTY", Some(if is_dirty { "true" } else { "false" }));

    tauri_build::build()
}

fn cargo_package_version() -> String {
    env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
}

fn read_env_override(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn emit_build_env(key: &str, value: Option<&str>) {
    let normalized = value.unwrap_or("");
    println!("cargo:rustc-env={key}={normalized}");
}

fn git_output(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn git_is_dirty(repo_root: &Path) -> bool {
    git_output(repo_root, &["status", "--porcelain"])
        .map(|output| !output.is_empty())
        .unwrap_or(false)
}

fn parse_truthy_flag(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

fn default_label_for_channel(channel: Option<&str>) -> Option<String> {
    let channel = channel?;
    let normalized = channel.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "beta" => Some("Beta".to_string()),
        "alpha" => Some("Alpha".to_string()),
        "nightly" => Some("Nightly".to_string()),
        "canary" => Some("Canary".to_string()),
        "preview" => Some("Preview".to_string()),
        "rc" => Some("RC".to_string()),
        "stable" => None,
        _ => None,
    }
}

fn sanitize_build_component(value: &str) -> Option<String> {
    let mut component = String::new();
    let mut last_was_dash = false;

    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            component.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if matches!(ch, '-' | '_' | ' ' | '.') {
            if !component.is_empty() && !last_was_dash {
                component.push('-');
                last_was_dash = true;
            }
        }
    }

    let component = component.trim_matches('-').to_string();
    if component.is_empty() {
        None
    } else {
        Some(component)
    }
}

fn compose_build_id(
    version: &str,
    version_label: Option<&str>,
    build_channel: Option<&str>,
    build_number: Option<&str>,
    git_sha: Option<&str>,
    is_dirty: bool,
) -> Option<String> {
    let mut components = Vec::new();

    if let Some(channel) = build_channel.and_then(sanitize_build_component) {
        components.push(channel);
    } else if let Some(label) = version_label.and_then(sanitize_build_component) {
        components.push(label);
    }

    if let Some(number) = build_number.and_then(sanitize_build_component) {
        components.push(number);
    }

    if let Some(sha) = git_sha.and_then(sanitize_build_component) {
        components.push(sha);
    }

    if is_dirty {
        components.push("dirty".to_string());
    }

    if components.is_empty() {
        None
    } else {
        Some(format!("{version}+{}", components.join(".")))
    }
}

fn register_git_rerun_paths(repo_root: &Path) {
    let Some(git_dir) = resolve_git_dir(repo_root) else {
        return;
    };

    let head_path = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());

    if let Ok(head_content) = fs::read_to_string(&head_path) {
        if let Some(reference) = head_content.strip_prefix("ref:") {
            let ref_path = git_dir.join(reference.trim());
            println!("cargo:rerun-if-changed={}", ref_path.display());
        }
    }

    let index_path = git_dir.join("index");
    println!("cargo:rerun-if-changed={}", index_path.display());
}

fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf> {
    let git_path = repo_root.join(".git");
    if git_path.is_dir() {
        return Some(git_path);
    }

    let git_file = fs::read_to_string(&git_path).ok()?;
    let git_dir = git_file.strip_prefix("gitdir:")?.trim();
    let git_dir_path = Path::new(git_dir);
    if git_dir_path.is_absolute() {
        Some(git_dir_path.to_path_buf())
    } else {
        Some(repo_root.join(git_dir_path))
    }
}
