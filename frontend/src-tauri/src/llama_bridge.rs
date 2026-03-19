use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tauri::{AppHandle, Manager, State};

#[derive(Clone)]
pub struct LlamaConfig {
    /// The filename within `<app_data_dir>/models/`, e.g. "llama-3.2-3b.gguf"
    pub model_filename: String,

    /// Runtime params (tune later)
    pub context_size: u32,
    pub max_tokens: i32,
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub stop_tokens: Vec<String>,
}

impl Default for LlamaConfig {
    fn default() -> Self {
        Self {
            model_filename: "model.gguf".to_string(),
            context_size: 2048,
            max_tokens: 512,
            temperature: 0.8,
            top_k: 64,
            top_p: 0.95,
            stop_tokens: vec![],
        }
    }
}

/// Tauri-managed state for the llama configuration.
/// Wrapped in RwLock to allow runtime updates when model selection changes.
pub struct LlamaState(pub RwLock<LlamaConfig>);

fn models_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow!("Failed to resolve app_data_dir: {}", e))?
        .join("models")
        .join("summary"); // GGUF models are stored in models/summary/

    Ok(dir)
}

fn resolve_model_path(app: &AppHandle, model_filename: &str) -> Result<PathBuf> {
    let path = models_dir(app)?.join(model_filename);

    // Optional but recommended: fail early with a clear error if missing.
    if !path.exists() {
        return Err(anyhow!(
            "Model not found at {}",
            path.display()
        ));
    }

    Ok(path)
}

pub fn run_llama(app: &AppHandle, state: &State<LlamaState>, prompt: &str) -> Result<String> {
    let cfg = state.0.read()
        .map_err(|e| anyhow!("Failed to acquire read lock on LlamaConfig: {}", e))?;

    let model_path = resolve_model_path(app, &cfg.model_filename)
        .with_context(|| "Unable to resolve model path")?;

    run_llama_impl(prompt, &model_path, &cfg)
}

/// Update the model filename in LlamaState
pub fn update_model_filename(state: &State<LlamaState>, model_filename: String) -> Result<()> {
    let mut cfg = state.0.write()
        .map_err(|e| anyhow!("Failed to acquire write lock on LlamaConfig: {}", e))?;
    cfg.model_filename = model_filename;
    Ok(())
}

/// Update the entire LlamaConfig
pub fn update_config(state: &State<LlamaState>, config: LlamaConfig) -> Result<()> {
    let mut cfg = state.0.write()
        .map_err(|e| anyhow!("Failed to acquire write lock on LlamaConfig: {}", e))?;
    *cfg = config;
    Ok(())
}

/// Get a copy of the current config
pub fn get_config(state: &State<LlamaState>) -> Result<LlamaConfig> {
    let cfg = state.0.read()
        .map_err(|e| anyhow!("Failed to acquire read lock on LlamaConfig: {}", e))?;
    Ok(cfg.clone())
}

#[cfg(target_os = "ios")]
fn run_llama_impl(prompt: &str, model_path: &Path, cfg: &LlamaConfig) -> Result<String> {
    // iOS: in-process (no subprocess, no stdin).
    // IMPORTANT: Do not call run_stdio_server() on iOS.
    llama_helper::generate_once(
        prompt,
        model_path
            .to_str()
            .ok_or_else(|| anyhow!("Model path is not valid UTF-8"))?,
        cfg.context_size,
        cfg.max_tokens,
        cfg.temperature,
        cfg.top_k,
        cfg.top_p,
        cfg.stop_tokens.clone(),
    )
}

#[cfg(not(target_os = "ios"))]
fn run_llama_impl(prompt: &str, _model_path: &Path, _cfg: &LlamaConfig) -> Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // Desktop: keep your current subprocess behavior.
    // If your helper expects JSON lines, build the JSON here.
    let mut child = Command::new("llama-helper")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn llama-helper")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
        stdin.write_all(b"\n")?;
    }

    let out = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
