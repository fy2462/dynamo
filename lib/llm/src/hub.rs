// SPDX-FileCopyrightText: Copyright (c) 2024-2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::env;
use std::path::{Path, PathBuf};

use modelexpress_client::{
    Client as MxClient, ClientConfig as MxClientConfig, ModelProvider as MxModelProvider,
};
use modelexpress_common::download as mx;

/// Example: export MODEL_EXPRESS_URL=http://localhost:8001
const MODEL_EXPRESS_ENDPOINT_ENV_VAR: &str = "MODEL_EXPRESS_URL";

/// Download a model using ModelExpress client. The client first requests for the model
/// from the server and fallbacks to direct download in case of server failure.
/// If ignore_weights is true, model weight files will be skipped
/// Returns the path to the model files
pub async fn from_hf(name: impl AsRef<Path>, ignore_weights: bool) -> anyhow::Result<PathBuf> {
    let name = name.as_ref();
    let model_name = name.display().to_string();

    let mut config: MxClientConfig = MxClientConfig::default();
    if let Ok(endpoint) = env::var(MODEL_EXPRESS_ENDPOINT_ENV_VAR) {
        config = config.with_endpoint(endpoint);
    }

    let result = match MxClient::new(config).await {
        Ok(mut client) => {
            tracing::info!("Successfully connected to ModelExpress server");
            match client
                .request_model_with_provider_and_fallback(
                    &model_name,
                    MxModelProvider::HuggingFace,
                    ignore_weights,
                )
                .await
            {
                Ok(()) => {
                    tracing::info!("Server download succeeded for model: {model_name}");
                    match client.get_model_path(&model_name).await {
                        Ok(path) => Ok(path),
                        Err(e) => {
                            tracing::warn!(
                                "Failed to resolve local model path after server download for '{model_name}': {e}. \
                                Falling back to direct download."
                            );
                            mx_download_direct(&model_name, ignore_weights).await
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Server download failed for model '{model_name}': {e}. Falling back to direct download."
                    );
                    mx_download_direct(&model_name, ignore_weights).await
                }
            }
        }
        Err(e) => {
            tracing::warn!("Cannot connect to ModelExpress server: {e}. Using direct download.");
            mx_download_direct(&model_name, ignore_weights).await
        }
    };

    match result {
        Ok(path) => {
            tracing::info!("ModelExpress download completed successfully for model: {model_name}");
            Ok(path)
        }
        Err(e) => {
            tracing::warn!("ModelExpress download failed for model '{model_name}': {e}");
            Err(e)
        }
    }
}

// Direct download using the ModelExpress client.
async fn mx_download_direct(model_name: &str, ignore_weights: bool) -> anyhow::Result<PathBuf> {
    let cache_dir = get_model_express_cache_dir();
    mx::download_model(
        model_name,
        MxModelProvider::HuggingFace,
        Some(cache_dir),
        ignore_weights,
    )
    .await
}

// TODO: remove in the future. This is a temporary workaround to find common
// cache directory between client and server.
fn get_model_express_cache_dir() -> PathBuf {
    // Check HF_HUB_CACHE environment variable
    // reference: https://huggingface.co/docs/huggingface_hub/en/package_reference/environment_variables#hfhubcache
    if let Ok(cache_path) = env::var("HF_HUB_CACHE") {
        return PathBuf::from(cache_path);
    }

    // Check HF_HOME environment variable (standard Hugging Face cache directory)
    // reference: https://huggingface.co/docs/huggingface_hub/en/package_reference/environment_variables#hfhome
    if let Ok(hf_home) = env::var("HF_HOME") {
        return PathBuf::from(hf_home).join("hub");
    }

    if let Ok(cache_path) = env::var("MODEL_EXPRESS_CACHE_PATH") {
        return PathBuf::from(cache_path);
    }


    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    PathBuf::from(home).join(".cache/huggingface/hub")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_from_hf_with_model_express() {
        let test_path = PathBuf::from("test-model");
        let _result: anyhow::Result<PathBuf> = from_hf(test_path, false).await;
    }

    #[test]
    fn test_get_model_express_cache_dir() {
        let cache_dir = get_model_express_cache_dir();
        assert!(!cache_dir.to_string_lossy().is_empty());
        assert!(cache_dir.is_absolute() || cache_dir.starts_with("."));
    }

    #[test]
    fn test_get_model_express_cache_dir_with_hf_home() {
        // Test that HF_HOME is respected when set
        unsafe {
            env::set_var("HF_HOME", "/custom/cache/path");
            let cache_dir = get_model_express_cache_dir();
            assert_eq!(cache_dir, PathBuf::from("/custom/cache/path/hub"));

            // Clean up
            env::remove_var("HF_HOME");
        }
    }
}
