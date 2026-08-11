use std::path::Path;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::ai::error::AiError;
use crate::ai::models::{TranscriptionOptions, TranscriptionResult};
use crate::ai::provider::AiProvider;

/// Extension pour les fournisseurs capables de transcrire de l'audio.
#[async_trait]
pub trait TranscriptionProvider: AiProvider {
    async fn transcribe(
        &self,
        api_key: &str,
        audio_path: &Path,
        options: TranscriptionOptions,
        cancel: &CancellationToken,
    ) -> Result<TranscriptionResult, AiError>;
}
