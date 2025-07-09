use std::process::Command;

fn main() {
    // Set build timestamp
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    println!("cargo:rustc-env=CLAUDE_TOKEN_MONITOR_BUILD_TIME={}", timestamp);
    
    // Rerun if git changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    
    // Try to get git commit hash if available
    if let Ok(output) = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let git_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=CLAUDE_TOKEN_MONITOR_GIT_HASH={}", git_hash);
        }
    }
}