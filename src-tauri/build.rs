use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

fn get_platform_info() -> Result<(String, String, String, String), Box<dyn std::error::Error>> {
    // Retrieve the current operating system and architecture
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    // Determine os_name, arch_name, filename, and download_filename based on OS and architecture
    let (os_name, arch_name, filename, download_filename) = match (os, arch) {
        ("macos", "aarch64") => (
            "darwin",
            "aarch64",
            "ollama-aarch64-apple-darwin",
            "ollama-darwin-aarch64"
        ),
        ("macos", "x86_64") => (
            "darwin",
            "x86_64",
            "ollama-x86_64-apple-darwin",
            "ollama-darwin-amd64"
        ),
        ("linux", "aarch64") => (
            "linux",
            "aarch64",
            "ollama-aarch64-unknown-linux",
            "ollama-linux-arm64"
        ),
        ("linux", "x86_64") => (
            "linux",
            "x86_64",
            "ollama-x86_64-unknown-linux",
            "ollama-linux-amd64"
        ),
        ("windows", _) => (
            "windows",
            arch,
            "ollama.exe",
            "ollama.exe"
        ),
        _ => return Err("Unsupported platform".into()),
    };

    Ok((
        os_name.to_string(),
        arch_name.to_string(),
        filename.to_string(),
        download_filename.to_string(),
    ))
}

fn download_ollama() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let (os_name, arch_name, filename, download_filename) = get_platform_info()?;
        
        let binary_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("binaries");
        fs::create_dir_all(&binary_dir)?;

        let target_path = binary_dir.join(&filename);
        
        // Only proceed if the exact platform-specific binary doesn't exist
        if !target_path.exists() {
            println!("Downloading Ollama for {}-{}...", os_name, arch_name);
            
            let url = format!(
                "https://github.com/ollama/ollama/releases/latest/download/{}",
                download_filename
            );
            println!("Downloading from URL: {}", url);

            let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;

            let mut file = File::create(&target_path).map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;

            // Make the binary executable on Unix-like systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&target_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&target_path, perms)?;
            }

            println!("Successfully downloaded Ollama for {}-{}", os_name, arch_name);
        } else {
            println!("Platform-specific Ollama binary already exists for {}-{}", os_name, arch_name);
        }

        Ok(())
    })
}

fn main() {
    // Download Ollama if needed
    if let Err(e) = download_ollama() {
        println!("cargo:warning=Failed to download Ollama: {}", e);
    }

    tauri_build::build()
} 