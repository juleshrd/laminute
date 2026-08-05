use async_trait::async_trait;

use crate::ai::error::AiError;
use crate::ai::models::{SummaryOptions, SummaryResult};
use crate::ai::provider::AiProvider;

/// Extension pour les fournisseurs capables de résumer du texte.
#[async_trait]
pub trait SummaryProvider: AiProvider {
    async fn summarize(
        &self,
        api_key: &str,
        text: &str,
        options: SummaryOptions,
    ) -> Result<SummaryResult, AiError>;
}
