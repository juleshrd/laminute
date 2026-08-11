//! Orchestration map-reduce pour comptes-rendus longs (JUL-198).

use tokio_util::sync::CancellationToken;

use crate::ai::error::AiError;
use crate::ai::jobs::AiJobState;
use crate::ai::models::{SummaryOptions, SummaryResult};
use crate::ai::registry::ProviderRegistry;
use crate::ai::structured_summary::{
    parse_structured_summary, StructuredSummary, SummaryPromptMode,
};
use crate::ai::token_pipeline::{
    estimate_cost_usd, estimate_tokens, effective_input_token_budget, merge_partial_summaries,
    needs_map_reduce_pipeline, split_transcription, SummaryPipelineMeta, PipelinePhase,
    DEFAULT_OVERLAP_SEGMENTS, PROMPT_OVERHEAD_TOKENS,
};
use crate::ai::token_pipeline::{token_budget_to_char_limit};

const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

pub struct SummaryRunResult {
    pub summary_result: SummaryResult,
    pub structured: StructuredSummary,
    pub meta: SummaryPipelineMeta,
}

pub type ProgressCallback = Box<dyn Fn(PipelinePhase) + Send + Sync>;

/// Génère un compte-rendu structuré : chemin direct si le texte tient dans la fenêtre, sinon map-reduce.
pub async fn run_structured_summary(
    registry: &ProviderRegistry,
    provider_id: &str,
    api_key: &str,
    transcription_text: &str,
    model: Option<String>,
    cancel: &CancellationToken,
    jobs: Option<&AiJobState>,
    job_id: Option<&str>,
    progress: Option<ProgressCallback>,
) -> Result<SummaryRunResult, AiError> {
    let max_tokens = DEFAULT_MAX_OUTPUT_TOKENS;
    let model_ref = model.as_deref();

    let estimated_input = estimate_tokens(transcription_text) as u64;
    let estimated_output = max_tokens as u64;
    let mut meta = SummaryPipelineMeta {
        estimated_input_tokens: Some(estimated_input),
        estimated_cost_usd: Some(estimate_cost_usd(
            provider_id,
            model_ref,
            estimated_input,
            estimated_output,
        )),
        ..Default::default()
    };

    if !needs_map_reduce_pipeline(transcription_text, provider_id, model_ref, max_tokens) {
        meta.pipeline_used = Some(false);
        meta.chunk_count = Some(1);
        return run_single_pass(
            registry,
            provider_id,
            api_key,
            transcription_text,
            model,
            max_tokens,
            SummaryPromptMode::Full,
            cancel,
            meta,
        )
        .await;
    }

    run_map_reduce_pipeline(
        registry,
        provider_id,
        api_key,
        transcription_text,
        model,
        max_tokens,
        cancel,
        jobs,
        job_id,
        progress,
        meta,
    )
    .await
}

async fn run_single_pass(
    registry: &ProviderRegistry,
    provider_id: &str,
    api_key: &str,
    text: &str,
    model: Option<String>,
    max_tokens: u32,
    prompt_mode: SummaryPromptMode,
    cancel: &CancellationToken,
    meta: SummaryPipelineMeta,
) -> Result<SummaryRunResult, AiError> {
    let summary_result = registry
        .summarize_text(
            provider_id,
            api_key,
            text,
            SummaryOptions {
                model,
                max_tokens: Some(max_tokens),
                prompt_mode,
            },
            cancel,
        )
        .await?;
    let structured = parse_structured_summary(&summary_result.text)?;
    Ok(SummaryRunResult {
        summary_result,
        structured,
        meta,
    })
}

async fn run_map_reduce_pipeline(
    registry: &ProviderRegistry,
    provider_id: &str,
    api_key: &str,
    transcription_text: &str,
    model: Option<String>,
    max_tokens: u32,
    cancel: &CancellationToken,
    jobs: Option<&AiJobState>,
    job_id: Option<&str>,
    progress: Option<ProgressCallback>,
    mut meta: SummaryPipelineMeta,
) -> Result<SummaryRunResult, AiError> {
    let model_ref = model.as_deref();
    let token_budget = effective_input_token_budget(provider_id, model_ref, max_tokens)
        .saturating_sub(PROMPT_OVERHEAD_TOKENS / 2);
    let char_limit = token_budget_to_char_limit(token_budget);

    emit_progress(&progress, PipelinePhase::Chunking);

    let chunks = split_transcription(
        transcription_text,
        char_limit,
        DEFAULT_OVERLAP_SEGMENTS,
    )?;
    let chunk_count = chunks.len() as u32;
    meta.pipeline_used = Some(true);
    meta.chunk_count = Some(chunk_count);

    let total_input_tokens: u64 = chunks
        .iter()
        .map(|c| estimate_tokens(c) as u64)
        .sum::<u64>()
        + estimate_tokens(transcription_text) as u64 / 4;
    let total_output_tokens = (chunk_count as u64 + 1) * max_tokens as u64;
    meta.estimated_input_tokens = Some(total_input_tokens);
    meta.estimated_cost_usd = Some(estimate_cost_usd(
        provider_id,
        model_ref,
        total_input_tokens,
        total_output_tokens,
    ));

    let mut partials = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        check_cancelled(jobs, job_id, cancel)?;

        let phase = PipelinePhase::Map {
            index: index + 1,
            total: chunks.len(),
        };
        emit_progress(&progress, phase);

        let result = registry
            .summarize_text(
                provider_id,
                api_key,
                chunk,
                SummaryOptions {
                    model: model.clone(),
                    max_tokens: Some(max_tokens),
                    prompt_mode: SummaryPromptMode::Partial,
                },
                cancel,
            )
            .await?;
        let partial = parse_structured_summary(&result.text)?;
        partials.push(partial);
    }

    check_cancelled(jobs, job_id, cancel)?;

    emit_progress(&progress, PipelinePhase::Reduce);

    let merged = merge_partial_summaries(&partials);

    if chunks.len() == 1 {
        let summary_result = SummaryResult {
            text: serde_json::to_string(&merged).map_err(|e| AiError::Other(e.to_string()))?,
            model: model.clone().unwrap_or_default(),
        };
        return Ok(SummaryRunResult {
            summary_result,
            structured: merged,
            meta,
        });
    }

    let reduce_input = serde_json::to_string(&partials)
        .map_err(|e| AiError::Other(format!("sérialisation des partiels : {e}")))?;

    if estimate_tokens(&reduce_input)
        > effective_input_token_budget(provider_id, model_ref, max_tokens)
    {
        merged.validate()?;
        let summary_result = SummaryResult {
            text: serde_json::to_string(&merged).map_err(|e| AiError::Other(e.to_string()))?,
            model: model.clone().unwrap_or_default(),
        };
        return Ok(SummaryRunResult {
            summary_result,
            structured: merged,
            meta,
        });
    }

    let summary_result = registry
        .summarize_text(
            provider_id,
            api_key,
            &reduce_input,
            SummaryOptions {
                model: model.clone(),
                max_tokens: Some(max_tokens),
                prompt_mode: SummaryPromptMode::Reduce,
            },
            cancel,
        )
        .await?;

    let structured = parse_structured_summary(&summary_result.text).unwrap_or_else(|_| merged.clone());

    structured.validate()?;

    Ok(SummaryRunResult {
        summary_result,
        structured,
        meta,
    })
}

fn check_cancelled(
    jobs: Option<&AiJobState>,
    job_id: Option<&str>,
    cancel: &CancellationToken,
) -> Result<(), AiError> {
    if cancel.is_cancelled() {
        return Err(AiError::Cancelled);
    }
    if let (Some(jobs), Some(job_id)) = (jobs, job_id) {
        jobs.ensure_not_cancelled(job_id)
            .map_err(|_| AiError::Cancelled)?;
    }
    Ok(())
}

fn emit_progress(progress: &Option<ProgressCallback>, phase: PipelinePhase) {
    if let Some(cb) = progress {
        cb(phase);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::capabilities::ProviderCapabilities;
    use crate::ai::models::{KeyValidationResult, ModelInfo};
    use crate::ai::provider::AiProvider;
    use crate::ai::summary::SummaryProvider;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingSummaryProvider {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AiProvider for CountingSummaryProvider {
        fn id(&self) -> &str {
            "stub-pipeline"
        }
        fn display_name(&self) -> &str {
            "Stub Pipeline"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                transcription: false,
                summary: true,
                local: true,
                streaming: false,
                diarization: false,
            }
        }
        async fn validate_key(&self, _api_key: &str) -> Result<KeyValidationResult, AiError> {
            Ok(KeyValidationResult {
                valid: true,
                message: "ok".into(),
                models: None,
            })
        }
        async fn list_models(&self, _api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl SummaryProvider for CountingSummaryProvider {
        async fn summarize(
            &self,
            _api_key: &str,
            text: &str,
            options: SummaryOptions,
            cancel: &CancellationToken,
        ) -> Result<SummaryResult, AiError> {
            if cancel.is_cancelled() {
                return Err(AiError::Cancelled);
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            let synthese = match options.prompt_mode {
                SummaryPromptMode::Partial => format!("partial:{}", text.chars().take(20).collect::<String>()),
                SummaryPromptMode::Reduce => "merged reduce".into(),
                SummaryPromptMode::Full => "full summary".into(),
            };
            let json = serde_json::json!({
                "synthese": synthese,
                "decisions": ["Décision test"],
                "actions": [],
                "risques": [],
                "questionsOuvertes": []
            });
            Ok(SummaryResult {
                text: json.to_string(),
                model: "stub".into(),
            })
        }
    }

    fn long_diarized_text(segment_count: usize, words_per_segment: usize) -> String {
        (0..segment_count)
            .map(|i| {
                let words = (0..words_per_segment)
                    .map(|w| format!("mot{w}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("[SPEAKER_{i:02} {i}.0s–{i}.5s] {words}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[tokio::test]
    async fn short_text_uses_single_pass() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = ProviderRegistry::new();
        let stub = Arc::new(CountingSummaryProvider {
            calls: calls.clone(),
        });
        registry.register_ai(stub.clone());
        registry.register_summary(stub);

        let result = run_structured_summary(
            &registry,
            "stub-pipeline",
            "",
            "Réunion courte.",
            None,
            &CancellationToken::new(),
            None,
            None,
            None,
        )
        .await
        .expect("single pass");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.meta.pipeline_used, Some(false));
        assert_eq!(result.structured.synthese, "full summary");
    }

    #[tokio::test]
    async fn long_text_triggers_map_reduce() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = ProviderRegistry::new();
        let stub = Arc::new(CountingSummaryProvider {
            calls: calls.clone(),
        });
        registry.register_ai(stub.clone());
        registry.register_summary(stub);

        let text = long_diarized_text(80, 40);
        let result = run_structured_summary(
            &registry,
            "stub-pipeline",
            "",
            &text,
            None,
            &CancellationToken::new(),
            None,
            None,
            None,
        )
        .await
        .expect("pipeline");

        assert!(calls.load(Ordering::SeqCst) >= 2);
        assert_eq!(result.meta.pipeline_used, Some(true));
        assert!(result.meta.chunk_count.unwrap_or(0) >= 2);
    }

    #[tokio::test]
    async fn cancellation_between_chunks_aborts_pipeline() {
        let calls = Arc::new(AtomicUsize::new(0));
        let cancel = CancellationToken::new();

        struct CancellingProvider {
            calls: Arc<AtomicUsize>,
            cancel: CancellationToken,
        }

        #[async_trait]
        impl AiProvider for CancellingProvider {
            fn id(&self) -> &str {
                "stub-pipeline"
            }
            fn display_name(&self) -> &str {
                "Stub"
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities {
                    transcription: false,
                    summary: true,
                    local: true,
                    streaming: false,
                    diarization: false,
                }
            }
            async fn validate_key(&self, _: &str) -> Result<KeyValidationResult, AiError> {
                Ok(KeyValidationResult {
                    valid: true,
                    message: "ok".into(),
                    models: None,
                })
            }
            async fn list_models(&self, _: &str) -> Result<Vec<ModelInfo>, AiError> {
                Ok(vec![])
            }
        }

        #[async_trait]
        impl SummaryProvider for CancellingProvider {
            async fn summarize(
                &self,
                _: &str,
                _text: &str,
                _: SummaryOptions,
                token: &CancellationToken,
            ) -> Result<SummaryResult, AiError> {
                let n = self.calls.fetch_add(1, Ordering::SeqCst);
                if n >= 1 {
                    self.cancel.cancel();
                }
                if token.is_cancelled() {
                    return Err(AiError::Cancelled);
                }
                let json = serde_json::json!({
                    "synthese": format!("chunk {n}"),
                    "decisions": [],
                    "actions": [],
                    "risques": [],
                    "questionsOuvertes": []
                });
                Ok(SummaryResult {
                    text: json.to_string(),
                    model: "stub".into(),
                })
            }
        }

        let mut registry = ProviderRegistry::new();
        let stub = Arc::new(CancellingProvider {
            calls: calls.clone(),
            cancel: cancel.clone(),
        });
        registry.register_ai(stub.clone());
        registry.register_summary(stub);

        let text = long_diarized_text(80, 40);
        let result = run_structured_summary(
            &registry,
            "stub-pipeline",
            "",
            &text,
            None,
            &cancel,
            None,
            None,
            None,
        )
        .await;

        assert!(matches!(result, Err(AiError::Cancelled)));
        assert!(calls.load(Ordering::SeqCst) >= 2);
    }
}
