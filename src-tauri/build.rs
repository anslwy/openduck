use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=../src/");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_LABEL");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_NUMBER");
    println!("cargo:rerun-if-env-changed=VERSION_LABEL");
    println!("cargo:rerun-if-env-changed=BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=BUILD_NUMBER");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_BUILD_ID");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_GIT_SHA");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_GIT_DIRTY");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_UPDATER_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=OPEN_DUCK_UPDATER_ENDPOINT");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest_dir.clone());
    let target = env::var("TARGET").unwrap_or_default();
    let updater_public_key_path = manifest_dir.join("updater-public-key.pem");

    register_git_rerun_paths(&repo_root);
    println!(
        "cargo:rerun-if-changed={}",
        updater_public_key_path.display()
    );
    build_apple_translation_helper(&manifest_dir, &target);

    let version =
        read_env_override("OPEN_DUCK_BUILD_VERSION").unwrap_or_else(|| cargo_package_version());
    let build_channel = read_env_override_aliases(&["OPEN_DUCK_BUILD_CHANNEL", "BUILD_CHANNEL"]);
    let build_number = read_env_override_aliases(&["OPEN_DUCK_BUILD_NUMBER", "BUILD_NUMBER"]);
    let version_label = read_env_override_aliases(&["OPEN_DUCK_BUILD_LABEL", "VERSION_LABEL"])
        .or_else(|| default_label_for_channel(build_channel.as_deref()));
    let has_git = resolve_git_dir(&repo_root).is_some();

    let git_sha = read_env_override("OPEN_DUCK_GIT_SHA").or_else(|| {
        if has_git {
            git_output(&repo_root, &["rev-parse", "HEAD"])
        } else {
            None
        }
    });
    let is_dirty = read_env_override("OPEN_DUCK_GIT_DIRTY")
        .map(|value| parse_truthy_flag(&value))
        .unwrap_or_else(|| has_git && git_is_dirty(&repo_root));
    let dirty_files = read_env_override("OPEN_DUCK_GIT_DIRTY_FILES").or_else(|| {
        if has_git {
            git_output(&repo_root, &["status", "--porcelain"])
        } else {
            None
        }
    });

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
    let updater_public_key = read_env_override("OPEN_DUCK_UPDATER_PUBLIC_KEY")
        .or_else(|| read_file_trimmed(&updater_public_key_path));
    let updater_endpoint =
        read_env_override("OPEN_DUCK_UPDATER_ENDPOINT").or_else(default_updater_endpoint);

    emit_build_env("OPEN_DUCK_BUILD_VERSION", Some(&version));
    emit_build_env("OPEN_DUCK_BUILD_LABEL", version_label.as_deref());
    emit_build_env("OPEN_DUCK_BUILD_CHANNEL", build_channel.as_deref());
    emit_build_env("OPEN_DUCK_BUILD_NUMBER", build_number.as_deref());
    emit_build_env("OPEN_DUCK_BUILD_ID", build_id.as_deref());
    emit_build_env("OPEN_DUCK_GIT_SHA", git_sha.as_deref());
    emit_build_env(
        "OPEN_DUCK_GIT_DIRTY",
        Some(if is_dirty { "true" } else { "false" }),
    );
    emit_build_env("OPEN_DUCK_GIT_DIRTY_FILES", dirty_files.as_deref());
    emit_build_env(
        "OPEN_DUCK_UPDATER_PUBLIC_KEY",
        updater_public_key.as_deref(),
    );
    emit_build_env("OPEN_DUCK_UPDATER_ENDPOINT", updater_endpoint.as_deref());

    tauri_build::build()
}

fn build_apple_translation_helper(manifest_dir: &Path, target: &str) {
    if !target.ends_with("apple-darwin") {
        return;
    }

    let source_path = manifest_dir
        .join("src")
        .join("apple_translation_helper.swift");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let sidecar_dir = manifest_dir.join("bin");
    let sidecar_path = sidecar_dir.join(format!("apple-translation-helper-{target}"));
    let helper_app_dir = manifest_dir
        .join("resources")
        .join("apple-translation-helper.app");
    let helper_contents_dir = helper_app_dir.join("Contents");
    let helper_macos_dir = helper_contents_dir.join("MacOS");
    let helper_executable_path = helper_macos_dir.join("apple-translation-helper");
    let build_script_path = manifest_dir.join("build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let module_cache_path = out_dir.join("swift-module-cache");

    if let Err(err) = fs::create_dir_all(&sidecar_dir) {
        panic!("failed to create helper sidecar directory: {err}");
    }
    if let Err(err) = fs::create_dir_all(&helper_macos_dir) {
        panic!("failed to create Apple translation helper app bundle: {err}");
    }
    if let Err(err) = fs::create_dir_all(&module_cache_path) {
        panic!("failed to create Swift module cache directory: {err}");
    }

    let swift_target = match target {
        "aarch64-apple-darwin" => "arm64-apple-macosx15.0",
        "x86_64-apple-darwin" => "x86_64-apple-macosx15.0",
        _ => return,
    };

    if file_is_stale(
        &helper_executable_path,
        &[source_path.as_path(), build_script_path.as_path()],
    ) {
        let mut swiftc = Command::new("swiftc");
        swiftc
            .arg("-Osize")
            .arg("-parse-as-library")
            .arg("-target")
            .arg(swift_target)
            .arg(&source_path)
            .arg("-o")
            .arg(&helper_executable_path)
            .env("CLANG_MODULE_CACHE_PATH", &module_cache_path);

        // Embed the Info.plist into the binary so it can function as a standalone signed app/sidecar
        let info_plist_path = helper_contents_dir.join("Info.plist");
        let info_plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>apple-translation-helper</string>
  <key>CFBundleIdentifier</key>
  <string>com.openduck.app.apple-translation-helper</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>OpenDuck Apple Translation Helper</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSBackgroundOnly</key>
  <true/>
</dict>
</plist>
"#;
        write_if_changed(&info_plist_path, info_plist_content.as_bytes());

        swiftc
            .arg("-Xlinker")
            .arg("-sectcreate")
            .arg("-Xlinker")
            .arg("__TEXT")
            .arg("-Xlinker")
            .arg("__info_plist")
            .arg("-Xlinker")
            .arg(&info_plist_path);

        let status = swiftc.status().unwrap_or_else(|err| {
            panic!("failed to run swiftc for Apple translation helper: {err}")
        });

        if !status.success() {
            panic!("failed to build Apple translation helper with swiftc");
        }

        // Apply an ad-hoc signature with hardened runtime.
        // Tauri's bundler will re-sign this with the real identity later, but it will preserve the hardened runtime flag.
        let _ = Command::new("codesign")
            .arg("-s")
            .arg("-")
            .arg("--options")
            .arg("runtime")
            .arg("--force")
            .arg(&helper_executable_path)
            .status();
    }

    // Instead of a bash script sidecar, use the binary itself.
    // This ensures Tauri's bundler recognizes it as an executable and signs it properly.
    if file_is_stale(&sidecar_path, &[&helper_executable_path]) {
        fs::copy(&helper_executable_path, &sidecar_path)
            .unwrap_or_else(|err| panic!("failed to copy helper binary to sidecar: {err}"));
    }

    #[cfg(unix)]
    {
        set_executable_permissions_if_needed(&helper_executable_path);
        set_executable_permissions_if_needed(&sidecar_path);
    }
}

fn modified_at(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn file_is_stale(output: &Path, inputs: &[&Path]) -> bool {
    let Some(output_modified) = modified_at(output) else {
        return true;
    };

    inputs
        .iter()
        .filter_map(|input| modified_at(input))
        .any(|input_modified| input_modified > output_modified)
}

fn write_if_changed(path: &Path, content: &[u8]) {
    if fs::read(path)
        .map(|existing| existing == content)
        .unwrap_or(false)
    {
        return;
    }

    fs::write(path, content)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
}

#[cfg(unix)]
fn set_executable_permissions_if_needed(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::metadata(path)
        .unwrap_or_else(|err| panic!("failed to inspect Apple translation helper: {err}"));
    let current_mode = metadata.permissions().mode() & 0o777;

    if current_mode == 0o755 {
        return;
    }

    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|err| panic!("failed to chmod Apple translation helper: {err}"));
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

fn read_env_override_aliases(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| read_env_override(key))
}

fn emit_build_env(key: &str, value: Option<&str>) {
    let normalized = value.unwrap_or("");
    println!("cargo:rustc-env={key}={normalized}");
}

fn read_file_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn default_updater_endpoint() -> Option<String> {
    Some("https://github.com/anslwy/openduck/releases/latest/download/latest.json".to_string())
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
