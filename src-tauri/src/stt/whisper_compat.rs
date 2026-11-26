use async_trait::async_trait;

use crate::error::AppError;

use super::silence::{
    accept_transcript_given_with_prefix_echo, log_decision, NoSpeechGuard, VoiceActivity,
};
use super::transcript::{normalize_transcript, split_language_tag};
use super::{SttConfig, SttProvider, TranscriptEvent};

/// Configuration for a Whisper-compatible HTTP file-upload STT provider.
#[derive(Debug)]
pub struct WhisperCompatConfig {
    /// Shown in logs; the speech preset name.
    pub provider_name: String,
    /// Full `.../audio/transcriptions` URL.
    pub endpoint: String,
    pub model: String,
    /// Ask for `verbose_json` (it carries the detected language) when auto-detecting. False for a
    /// server known to refuse it: the built-in speech server (plan `language-model-suggestions`)
    /// answers only plain JSON, and Qwen3-ASR's own language tag gives the language anyway.
    pub verbose_json: bool,
    /// Enable runtime guards specific to OraType's built-in `llama-server` (Qwen3-ASR) speech
    /// provider (`temperature=0`, duration-scaled `max_tokens`, duration-scaled request timeout
    /// `max(60, ceil(30 + 1.5 * dur_s))` s with no timeout retries in the tested range,
    /// prompt-echo filtering, and privacy-safe output-cap logging).
    ///
    /// Must stay `false` for all external OpenAI / Groq / Whisper-compatible servers.
    pub builtin_llama_server: bool,
}

/// Plan `language-prompt-library`: endpoints that refused `response_format=verbose_json` in this
/// session. They get plain JSON (no detected language) from then on.
static NO_VERBOSE_JSON: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn verbose_json_refused(endpoint: &str) -> bool {
    NO_VERBOSE_JSON
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .any(|known| known == endpoint)
}

fn remember_verbose_json_refused(endpoint: &str) {
    let mut refused = NO_VERBOSE_JSON
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if !refused.iter().any(|known| known == endpoint) {
        refused.push(endpoint.to_string());
    }
}

/// A transcription answer: the text, and the language when the server reported one.
#[derive(Debug, Clone, PartialEq)]
struct Answer {
    text: Option<String>,
    language: Option<String>,
    /// Plan `qwen3-asr-support`: the text carried Qwen3-ASR's language tag.
    qwen3_asr: bool,
    hit_output_cap: bool,
}

/// Max audio buffer: ~24 MB PCM ≈ 12.5 min at 16kHz 16-bit mono.
/// Keeps the resulting WAV under 25 MB, the usual upload limit of OpenAI-compatible servers.
const MAX_AUDIO_BYTES: usize = 24 * 1024 * 1024;
const DEFAULT_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const DEFAULT_RETRY_BACKOFF_MS: u64 = 1000;
// Above the tested range the 0.6B model can fill its context and enter a decoding loop.
// Keep the existing timeout policy for those uploads until speech chunking is available.
const SMALL_MODEL_SCALED_TIMEOUT_MAX_SECS: f64 = 400.0;

/// Duration-scaled output token cap for built-in `llama-server` (Qwen3-ASR 1.7B & 0.6B):
/// `max(32, ceil(dur_s) * 16 + 32)`, intentionally without a context-window cap (`ctx_cap`).
///
/// Validated across `<= 30 s` dictation and `<= 600 s` long audio with `0` truncations while
/// eliminating all `0.6B` >65 s timeouts. Pause-based chunking for recordings longer than
/// `300-360 s` is deferred to a follow-up PR.
pub(crate) fn builtin_llama_max_tokens(pcm_bytes: usize, sample_rate: u32) -> u32 {
    let samples = pcm_bytes / 2;
    let rate = sample_rate.max(1) as usize;
    let ceil_secs = samples.div_ceil(rate) as u64;
    let scaled = ceil_secs.saturating_mul(16).saturating_add(32);
    scaled.max(32).min(u64::from(u32::MAX)) as u32
}

/// Duration-scaled request timeout for built-in `llama-server` (`builtin_llama_server == true`):
/// `max(60, ceil(30.0 + 1.5 * dur_s))` seconds.
///
/// Short recordings (`<= 20 s`) stay at 60 s; longer recordings scale linearly (`60 s -> 120 s`,
/// `180 s -> 300 s`, `600 s -> 930 s`, `720 s -> 1110 s`) to prevent premature timeouts on
/// slower CPUs or entry-level GPUs once timeout retries are disabled. External servers keep
/// [`DEFAULT_REQUEST_TIMEOUT`] (60 s) unchanged.
fn builtin_llama_request_timeout(dur_s: f64) -> std::time::Duration {
    let min_secs = DEFAULT_REQUEST_TIMEOUT.as_secs();
    if !dur_s.is_finite() || dur_s <= 0.0 {
        return DEFAULT_REQUEST_TIMEOUT;
    }
    let scaled = 30.0 + 1.5 * dur_s;
    if !scaled.is_finite() {
        return DEFAULT_REQUEST_TIMEOUT;
    }
    std::time::Duration::from_secs((scaled.ceil() as u64).max(min_secs))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CapHitMetrics {
    max_tokens: u32,
    output_tokens: Option<u64>,
    finish_reason_length: bool,
}

fn detect_output_cap_hit(v: &serde_json::Value, max_tokens: u32) -> Option<CapHitMetrics> {
    let output_tokens = v["usage"]["output_tokens"]
        .as_u64()
        .or_else(|| v["usage"]["completion_tokens"].as_u64());
    let finish_reason_length = v["finish_reason"].as_str() == Some("length")
        || v["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .and_then(|choice| choice["finish_reason"].as_str())
            == Some("length");
    let reached_token_cap = output_tokens.is_some_and(|out| out >= u64::from(max_tokens));
    (reached_token_cap || finish_reason_length).then_some(CapHitMetrics {
        max_tokens,
        output_tokens,
        finish_reason_length,
    })
}

/// Provider for any OpenAI Whisper-compatible transcription API
/// (whisper.cpp `whisper-server`, Speaches, and similar servers).
pub struct WhisperCompatProvider {
    provider_config: WhisperCompatConfig,
    stt_config: Option<SttConfig>,
    audio_buffer: Vec<u8>,
    client: reqwest::Client,
    #[cfg(test)]
    request_timeout_override: Option<std::time::Duration>,
    #[cfg(test)]
    retry_backoff_base_ms: u64,
    upload_probe: Option<crate::timing::UploadProbe>,
    detected_language: Option<String>,
    answered_as_qwen3_asr: bool,
}

impl WhisperCompatProvider {
    pub fn new(provider_config: WhisperCompatConfig) -> Self {
        Self::with_client(provider_config, reqwest::Client::new())
    }

    pub fn with_client(provider_config: WhisperCompatConfig, client: reqwest::Client) -> Self {
        Self {
            provider_config,
            stt_config: None,
            audio_buffer: Vec::new(),
            client,
            #[cfg(test)]
            request_timeout_override: None,
            #[cfg(test)]
            retry_backoff_base_ms: DEFAULT_RETRY_BACKOFF_MS,
            upload_probe: None,
            detected_language: None,
            answered_as_qwen3_asr: false,
        }
    }

    #[cfg(test)]
    fn with_request_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.request_timeout_override = Some(timeout);
        self
    }

    #[cfg(test)]
    fn with_retry_backoff_ms(mut self, base_ms: u64) -> Self {
        self.retry_backoff_base_ms = base_ms;
        self
    }

    fn uses_scaled_timeout(&self, audio_len_secs: f64) -> bool {
        self.provider_config.builtin_llama_server
            && (self.provider_config.model != super::models::QWEN3_ASR_SMALL_MODEL_ID
                || audio_len_secs <= SMALL_MODEL_SCALED_TIMEOUT_MAX_SECS)
    }

    fn request_timeout_for(&self, audio_len_secs: f64) -> std::time::Duration {
        #[cfg(test)]
        if let Some(override_timeout) = self.request_timeout_override {
            return override_timeout;
        }
        if self.uses_scaled_timeout(audio_len_secs) {
            builtin_llama_request_timeout(audio_len_secs)
        } else {
            DEFAULT_REQUEST_TIMEOUT
        }
    }

    fn retry_delay(&self, attempt: u32) -> std::time::Duration {
        #[cfg(test)]
        let base_ms = self.retry_backoff_base_ms;
        #[cfg(not(test))]
        let base_ms = DEFAULT_RETRY_BACKOFF_MS;
        std::time::Duration::from_millis(base_ms * 2u64.pow(attempt - 1))
    }

    /// Build a WAV file from raw PCM 16-bit mono audio. Public so test helpers can reuse it.
    pub fn build_wav(pcm: &[u8], sample_rate: u32) -> Vec<u8> {
        let data_len = pcm.len() as u32;
        let channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * (channels as u32) * (bits_per_sample as u32) / 8;
        let block_align = channels * bits_per_sample / 8;
        let file_size = 36 + data_len;

        let mut wav = Vec::with_capacity(44 + pcm.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend_from_slice(pcm);
        wav
    }
}

#[async_trait]
impl SttProvider for WhisperCompatProvider {
    async fn connect(&mut self, config: &SttConfig) -> Result<(), AppError> {
        self.stt_config = Some(config.clone());
        self.audio_buffer.clear();
        tracing::info!(
            "{} provider ready (buffering mode)",
            self.provider_config.provider_name
        );
        Ok(())
    }

    async fn send_audio(&mut self, chunk: &[u8]) -> Result<(), AppError> {
        if self.audio_buffer.len() + chunk.len() > MAX_AUDIO_BYTES {
            return Err(AppError::Config(format!(
                "{}: audio exceeds maximum length (~12 min)",
                self.provider_config.provider_name
            )));
        }
        self.audio_buffer.extend_from_slice(chunk);
        Ok(())
    }

    async fn recv_transcript(&mut self) -> Result<Option<TranscriptEvent>, AppError> {
        // File-based providers transcribe in disconnect(); keep this future
        // pending so the pipeline select loop does not busy-spin while recording.
        std::future::pending().await
    }

    async fn disconnect(&mut self) -> Result<Option<String>, AppError> {
        self.detected_language = None;
        self.answered_as_qwen3_asr = false;
        let config = match &self.stt_config {
            Some(c) => c.clone(),
            None => return Ok(None),
        };

        if self.audio_buffer.is_empty() {
            tracing::info!(
                "{}: no audio buffered, skipping",
                self.provider_config.provider_name
            );
            return Ok(None);
        }

        if let Some(probe) = &self.upload_probe {
            probe.note_audio(self.audio_buffer.len(), config.sample_rate);
        }
        let activity = VoiceActivity::measure(&self.audio_buffer, config.sample_rate);
        if !activity.has_speech() {
            log_decision(
                &self.provider_config.provider_name,
                &activity,
                Some(NoSpeechGuard::VoiceCheck),
            );
            self.audio_buffer.clear();
            return Ok(None);
        }

        let pcm_bytes = self.audio_buffer.len();
        let audio_len_secs = pcm_bytes as f64 / (config.sample_rate as f64 * 2.0);
        let max_tokens = self
            .provider_config
            .builtin_llama_server
            .then(|| builtin_llama_max_tokens(pcm_bytes, config.sample_rate));
        let wav_data = Self::build_wav(&self.audio_buffer, config.sample_rate);
        if let Some(probe) = &self.upload_probe {
            probe.mark_started(wav_data.len());
        }
        self.audio_buffer.clear();
        tracing::info!(
            "{}: sending {:.1}s of audio for transcription",
            self.provider_config.provider_name,
            audio_len_secs
        );

        // Plan `language-prompt-library`: with auto-detect, ask for `verbose_json`, which
        // carries the detected language; a server that refuses it gets plain JSON again.
        let verbose = self.provider_config.verbose_json
            && config.language.is_none()
            && !verbose_json_refused(&self.provider_config.endpoint);
        let retry_copy = verbose.then(|| wav_data.clone());
        let mut result = self
            .upload_wav(&config, wav_data, verbose, audio_len_secs, max_tokens)
            .await;
        if let Some(wav_data) = retry_copy {
            if let Err(AppError::Api { status, .. }) = &result {
                if matches!(status, 400 | 415 | 422) {
                    tracing::info!(
                        "{}: the server refused verbose_json (HTTP {status}); using plain JSON",
                        self.provider_config.provider_name
                    );
                    remember_verbose_json_refused(&self.provider_config.endpoint);
                    result = self
                        .upload_wav(&config, wav_data, false, audio_len_secs, max_tokens)
                        .await;
                }
            }
        }
        if let Some(probe) = &self.upload_probe {
            probe.mark_finished();
        }
        let answer = result?;
        self.detected_language = answer.language.clone();
        self.answered_as_qwen3_asr = answer.qwen3_asr;
        if let Some(language) = &answer.language {
            tracing::info!(
                "{}: detected language {language}",
                self.provider_config.provider_name
            );
        }
        // llama-server can read its default instruction back on short bursts of noise.
        if self.provider_config.builtin_llama_server
            && activity.voiced_ms < super::hallucination::MAX_VOICED_MS
            && answer.text.as_deref().is_some_and(|text| {
                super::hallucination::normalise(text) == "transcribeaudiototext"
            })
        {
            log_decision(
                &self.provider_config.provider_name,
                &activity,
                Some(NoSpeechGuard::PromptEcho),
            );
            return Ok(None);
        }
        Ok(accept_transcript_given_with_prefix_echo(
            &self.provider_config.provider_name,
            &activity,
            answer.text,
            &config.vocabulary,
            answer.hit_output_cap,
        ))
    }

    fn name(&self) -> &str {
        &self.provider_config.provider_name
    }

    fn set_upload_probe(&mut self, probe: crate::timing::UploadProbe) {
        self.upload_probe = Some(probe);
    }

    fn detected_language(&self) -> Option<String> {
        self.detected_language.clone()
    }

    fn answered_as_qwen3_asr(&self) -> bool {
        self.answered_as_qwen3_asr
    }
}

impl WhisperCompatProvider {
    /// Sends the WAV file, retrying server errors up to two times. Requests using the default
    /// timeout policy also retry timeouts. `verbose` asks for `verbose_json`, whose
    /// `language` field says what the server heard.
    async fn upload_wav(
        &self,
        config: &SttConfig,
        wav_data: Vec<u8>,
        verbose: bool,
        audio_len_secs: f64,
        max_tokens: Option<u32>,
    ) -> Result<Answer, AppError> {
        let timeout = self.request_timeout_for(audio_len_secs);
        let retry_timeouts = !self.uses_scaled_timeout(audio_len_secs);
        let mut attempt = 0u32;
        loop {
            let file_part = reqwest::multipart::Part::bytes(wav_data.clone())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| AppError::Config(e.to_string()))?;

            let mut form = reqwest::multipart::Form::new()
                .text("model", self.provider_config.model.to_string())
                .part("file", file_part);

            // Language hint. `None` means auto-detect, so the field is left out.
            if let Some(ref lang) = config.language {
                form = form.text("language", lang.clone());
            }
            if verbose {
                form = form.text("response_format", "verbose_json");
            }
            // Plan `dictionary-to-speech`: the words to expect, as OpenAI's `prompt` field.
            if let Some(prompt) = super::vocabulary::speech_prompt(&config.vocabulary) {
                form = form.text("prompt", prompt);
            }
            if let Some(max_tokens) = max_tokens {
                form = form
                    .text("temperature", "0")
                    .text("max_tokens", max_tokens.to_string());
            }

            let mut request = self
                .client
                .post(&self.provider_config.endpoint)
                .multipart(form)
                .timeout(timeout);

            if !config.api_key.trim().is_empty() {
                request = request.header("Authorization", format!("Bearer {}", config.api_key));
            }

            let request_started = std::time::Instant::now();
            let resp_result = request.send().await;
            tracing::info!(
                "{}: POST {} answered after {} ms ({})",
                self.provider_config.provider_name,
                self.provider_config.endpoint,
                request_started.elapsed().as_millis(),
                match &resp_result {
                    Ok(resp) => format!("HTTP {}", resp.status().as_u16()),
                    Err(error) => format!("error: {error}"),
                }
            );

            match resp_result {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    if status.is_success() {
                        let v: serde_json::Value = serde_json::from_str(&body)
                            .map_err(|e| AppError::Config(e.to_string()))?;
                        let cap_hit = max_tokens.and_then(|cap| detect_output_cap_hit(&v, cap));
                        if let Some(hit) = cap_hit {
                            tracing::warn!(
                                provider = %self.provider_config.provider_name,
                                dur_s = format_args!("{audio_len_secs:.2}"),
                                max_tokens = hit.max_tokens,
                                output_tokens = ?hit.output_tokens,
                                finish_reason_length = hit.finish_reason_length,
                                hit_cap = true,
                                "built-in llama-server transcription hit max_tokens cap"
                            );
                        }
                        // Plan `qwen3-asr-support`: Qwen3-ASR servers put the language in
                        // front of the text; a `language` field, when present, wins.
                        let (body, tagged) = split_language_tag(v["text"].as_str().unwrap_or(""));
                        let text = normalize_transcript(body);
                        let language = v["language"]
                            .as_str()
                            .and_then(super::normalize_detected_language)
                            .or_else(|| tagged.and_then(super::normalize_detected_language));

                        tracing::info!(
                            "{} transcription: {} chars",
                            self.provider_config.provider_name,
                            text.len()
                        );

                        return Ok(Answer {
                            text: (!text.is_empty()).then_some(text),
                            language,
                            qwen3_asr: tagged.is_some(),
                            hit_output_cap: cap_hit.is_some(),
                        });
                    } else if status.as_u16() >= 500 && attempt < 2 {
                        let truncate_at = body
                            .char_indices()
                            .take_while(|&(i, _)| i < 200)
                            .last()
                            .map(|(i, c)| i + c.len_utf8())
                            .unwrap_or(body.len());
                        tracing::warn!(
                            "{} server error {} (attempt {}/3): {}",
                            self.provider_config.provider_name,
                            status,
                            attempt + 1,
                            &body[..truncate_at]
                        );
                        attempt += 1;
                        tokio::time::sleep(self.retry_delay(attempt)).await;
                        continue;
                    } else {
                        // Truncate at a valid UTF-8 char boundary to avoid panic on multi-byte chars
                        let truncate_at = body
                            .char_indices()
                            .take_while(|&(i, _)| i < 200)
                            .last()
                            .map(|(i, c)| i + c.len_utf8())
                            .unwrap_or(body.len());
                        let sanitized = &body[..truncate_at];
                        // A refused verbose_json is expected once per server (the caller asks
                        // again with plain JSON), so it is not an error in the log.
                        if verbose && matches!(status.as_u16(), 400 | 415 | 422) {
                            tracing::debug!(
                                "{} HTTP {} for verbose_json: {}",
                                self.provider_config.provider_name,
                                status,
                                sanitized
                            );
                        } else {
                            tracing::error!(
                                "{} HTTP {}: {}",
                                self.provider_config.provider_name,
                                status,
                                sanitized
                            );
                        }
                        return Err(AppError::Api {
                            status: status.as_u16(),
                            body: sanitized.to_string(),
                        });
                    }
                }
                Err(e) if e.is_timeout() && retry_timeouts && attempt < 2 => {
                    tracing::warn!(
                        "{} timeout (attempt {}/3)",
                        self.provider_config.provider_name,
                        attempt + 1
                    );
                    attempt += 1;
                    tokio::time::sleep(self.retry_delay(attempt)).await;
                    continue;
                }
                Err(e) if e.is_timeout() => {
                    tracing::warn!(
                        "{} timeout (attempt {}/3); not retrying",
                        self.provider_config.provider_name,
                        attempt + 1
                    );
                    return Err(e.into());
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::super::silence::pcm_tone;
    use super::*;

    #[tokio::test]
    async fn connect_allows_empty_api_key() {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "local-whisper".to_string(),
            endpoint: "http://localhost:8000/v1/audio/transcriptions".to_string(),
            model: "test-model".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });

        let result = provider
            .connect(&SttConfig {
                api_key: String::new(),
                language: None,
                sample_rate: 16000,
                ..SttConfig::default()
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn disconnect_marks_the_upload_on_the_probe_even_when_it_fails() {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "test-whisper".to_string(),
            // Port 9 on localhost refuses the connection at once, so no retry wait.
            endpoint: "http://127.0.0.1:9/v1/audio/transcriptions".to_string(),
            model: "test-model".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });
        let probe = crate::timing::UploadProbe::default();
        provider.set_upload_probe(probe.clone());
        provider.connect(&SttConfig::default()).await.unwrap();
        // A tone between two quiet stretches passes the voice check.
        let mut audio = vec![0u8; 16_000];
        audio.extend(pcm_tone(0.3, 1.0, 16_000));
        audio.extend(vec![0u8; 16_000]);
        provider.send_audio(&audio).await.unwrap();

        assert!(provider.disconnect().await.is_err());

        let marks = probe.snapshot();
        assert!(marks.started_at.is_some());
        assert!(marks.finished_at.is_some());
        assert_eq!(marks.pcm_bytes, 64_000);
        assert_eq!(marks.wav_bytes, 64_044);
        assert!((marks.recording_secs() - 2.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn disconnect_without_audio_leaves_the_probe_empty() {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "test-whisper".to_string(),
            endpoint: "http://127.0.0.1:9/v1/audio/transcriptions".to_string(),
            model: "test-model".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });
        let probe = crate::timing::UploadProbe::default();
        provider.set_upload_probe(probe.clone());
        provider.connect(&SttConfig::default()).await.unwrap();

        assert!(matches!(provider.disconnect().await, Ok(None)));
        assert_eq!(probe.snapshot(), crate::timing::UploadMarks::default());
    }

    #[tokio::test]
    async fn silent_recording_notes_its_length_but_no_upload() {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "test-whisper".to_string(),
            endpoint: "http://127.0.0.1:9/v1/audio/transcriptions".to_string(),
            model: "test-model".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });
        let probe = crate::timing::UploadProbe::default();
        provider.set_upload_probe(probe.clone());
        provider.connect(&SttConfig::default()).await.unwrap();
        provider.send_audio(&[0u8; 64_000]).await.unwrap();

        assert!(matches!(provider.disconnect().await, Ok(None)));
        let marks = probe.snapshot();
        assert!(marks.started_at.is_none());
        assert!(marks.finished_at.is_none());
        assert_eq!(marks.wav_bytes, 0);
        assert!((marks.recording_secs() - 2.0).abs() < f64::EPSILON);
    }

    /// A transcription server that answers `verbose_json` requests with `verbose` and others
    /// with `plain` (status, body). Returns the endpoint and how many requests asked for it.
    async fn language_server(
        verbose: (u16, &'static str),
        plain: (u16, &'static str),
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/v1/audio/transcriptions",
            listener.local_addr().unwrap()
        );
        let verbose_hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = verbose_hits.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let counter = counter.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0u8; 65536];
                    let header_end = loop {
                        let Ok(n) = socket.read(&mut buf).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        request.extend_from_slice(&buf[..n]);
                        if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                            break at + 4;
                        }
                    };
                    let headers = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse().ok())
                        .unwrap_or(0);
                    while request.len() < header_end + length {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => request.extend_from_slice(&buf[..n]),
                        }
                    }
                    let asks_verbose = request.windows(12).any(|w| w == b"verbose_json");
                    let (status, body) = if asks_verbose {
                        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        verbose
                    } else {
                        plain
                    };
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                });
            }
        });
        (url, verbose_hits)
    }

    /// A transcription server that answers every request with `body` and keeps the last
    /// request's bytes.
    async fn recording_server(
        body: impl Into<String>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let body = body.into();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/v1/audio/transcriptions",
            listener.local_addr().unwrap()
        );
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let keep = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut request = Vec::new();
                let mut buf = [0u8; 65536];
                let header_end = loop {
                    let Ok(n) = socket.read(&mut buf).await else {
                        break None;
                    };
                    if n == 0 {
                        break None;
                    }
                    request.extend_from_slice(&buf[..n]);
                    if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        break Some(at + 4);
                    }
                };
                let Some(header_end) = header_end else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse().ok())
                    .unwrap_or(0);
                while request.len() < header_end + length {
                    match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => request.extend_from_slice(&buf[..n]),
                        _ => break,
                    }
                }
                *keep.lock().unwrap() = request;
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            }
        });
        (url, seen)
    }

    async fn transcribe_given(endpoint: &str, vocabulary: &[&str]) -> Option<String> {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "test-whisper".to_string(),
            endpoint: endpoint.to_string(),
            model: "test-model".to_string(),
            verbose_json: false,
            builtin_llama_server: false,
        });
        provider
            .connect(&SttConfig {
                vocabulary: vocabulary.iter().map(|w| w.to_string()).collect(),
                ..SttConfig::default()
            })
            .await
            .unwrap();
        let mut audio = vec![0u8; 16_000];
        audio.extend(pcm_tone(0.3, 1.0, 16_000));
        audio.extend(vec![0u8; 16_000]);
        provider.send_audio(&audio).await.unwrap();
        provider.disconnect().await.unwrap()
    }

    /// Plan `dictionary-to-speech`: the dictionary goes as the `prompt` field, only when there
    /// is one, and a transcript that is only those words is dropped.
    #[tokio::test]
    async fn the_dictionary_goes_as_the_prompt_field() {
        let (url, seen) = recording_server(r#"{"text":"Ask Sennett about OraType."}"#).await;
        let text = transcribe_given(&url, &["OraType", "Sennett"]).await;
        assert_eq!(text.as_deref(), Some("Ask Sennett about OraType."));
        let request = String::from_utf8_lossy(&seen.lock().unwrap()).to_string();
        assert!(request.contains("name=\"prompt\"\r\n\r\nOraType, Sennett\r\n"));

        transcribe_given(&url, &[]).await;
        let request = String::from_utf8_lossy(&seen.lock().unwrap()).to_string();
        assert!(!request.contains("name=\"prompt\""));

        let (echo, _) = recording_server(r#"{"text":"OraType, Sennett."}"#).await;
        assert_eq!(transcribe_given(&echo, &["OraType", "Sennett"]).await, None);
    }

    async fn transcribe_with(endpoint: &str, language: Option<&str>) -> WhisperCompatProvider {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "test-whisper".to_string(),
            endpoint: endpoint.to_string(),
            model: "test-model".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });
        provider
            .connect(&SttConfig {
                language: language.map(str::to_string),
                ..SttConfig::default()
            })
            .await
            .unwrap();
        let mut audio = vec![0u8; 16_000];
        audio.extend(pcm_tone(0.3, 1.0, 16_000));
        audio.extend(vec![0u8; 16_000]);
        provider.send_audio(&audio).await.unwrap();
        let text = provider.disconnect().await.unwrap();
        assert_eq!(text.as_deref(), Some("Hello there"));
        provider
    }

    /// Plan `language-prompt-library`: with auto-detect the provider asks for verbose_json and
    /// passes the language on; a full name becomes its code.
    #[tokio::test]
    async fn verbose_json_reports_the_detected_language() {
        let (url, verbose_hits) = language_server(
            (
                200,
                r#"{"text":"Hello there","language":"english","segments":[]}"#,
            ),
            (200, r#"{"text":"Hello there"}"#),
        )
        .await;
        let provider = transcribe_with(&url, None).await;
        assert_eq!(provider.detected_language().as_deref(), Some("en"));
        assert!(!provider.answered_as_qwen3_asr());
        assert_eq!(verbose_hits.load(std::sync::atomic::Ordering::SeqCst), 1);

        // A fixed language needs no detection: plain JSON, no detected language.
        let provider = transcribe_with(&url, Some("en")).await;
        assert_eq!(provider.detected_language(), None);
        assert_eq!(verbose_hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    /// Plan `qwen3-asr-support`: a Qwen3-ASR answer loses its language tag, and the tag's
    /// language counts as detected when the answer has no `language` field.
    #[tokio::test]
    async fn a_qwen3_asr_language_tag_is_removed_and_used_as_the_language() {
        let (url, _) = language_server(
            (
                200,
                r#"{"text":"language Cantonese<asr_text>Hello there","usage":{"type":"tokens"}}"#,
            ),
            (200, r#"{"text":"language English<asr_text>Hello there"}"#),
        )
        .await;
        let provider = transcribe_with(&url, None).await;
        assert_eq!(provider.detected_language().as_deref(), Some("yue"));
        assert!(provider.answered_as_qwen3_asr());

        // With a fixed language the tag is still removed.
        let provider = transcribe_with(&url, Some("en")).await;
        assert_eq!(provider.detected_language().as_deref(), Some("en"));
        assert!(provider.answered_as_qwen3_asr());
    }

    #[tokio::test]
    async fn a_server_that_refuses_verbose_json_gets_plain_json_from_then_on() {
        let (url, verbose_hits) = language_server(
            (400, r#"{"error":"response_format not supported"}"#),
            (200, r#"{"text":"Hello there"}"#),
        )
        .await;
        let provider = transcribe_with(&url, None).await;
        assert_eq!(provider.detected_language(), None);
        assert_eq!(verbose_hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        // The next recording does not ask again.
        transcribe_with(&url, None).await;
        assert_eq!(verbose_hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn recv_transcript_waits_for_file_based_provider() {
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "test-whisper".to_string(),
            endpoint: "https://example.test/transcriptions".to_string(),
            model: "test-model".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            provider.recv_transcript(),
        )
        .await;

        assert!(result.is_err());
    }

    fn test_audio_2s() -> Vec<u8> {
        let mut audio = vec![0u8; 16_000];
        audio.extend(pcm_tone(0.3, 1.0, 16_000));
        audio.extend(vec![0u8; 16_000]);
        audio
    }

    /// A server that executes `handler(attempt_index)` (1-based) on each request after reading
    /// the full HTTP request body.
    async fn scripted_server(
        handler: impl Fn(usize) -> (std::time::Duration, u16, String) + Send + Sync + 'static,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/v1/audio/transcriptions",
            listener.local_addr().unwrap()
        );
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = hits.clone();
        let handler = std::sync::Arc::new(handler);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let counter = counter.clone();
                let handler = handler.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0u8; 65536];
                    let header_end = loop {
                        let Ok(n) = socket.read(&mut buf).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        request.extend_from_slice(&buf[..n]);
                        if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                            break at + 4;
                        }
                    };
                    let headers = String::from_utf8_lossy(&request[..header_end]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse().ok())
                        .unwrap_or(0);
                    while request.len() < header_end + length {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => request.extend_from_slice(&buf[..n]),
                        }
                    }
                    let attempt = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    let (delay, status, body) = handler(attempt);
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                });
            }
        });
        (url, hits)
    }

    #[derive(Clone, Default)]
    struct TestLogBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl TestLogBuffer {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    impl std::io::Write for TestLogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn builtin_llama_max_tokens_scales_linearly_without_ctx_cap() {
        assert_eq!(builtin_llama_max_tokens(0, 16_000), 32);
        assert_eq!(builtin_llama_max_tokens(2, 16_000), 48);
        assert_eq!(builtin_llama_max_tokens(32_000, 16_000), 48);
        assert_eq!(builtin_llama_max_tokens(32_001, 16_000), 48);
        assert_eq!(builtin_llama_max_tokens(32_002, 16_000), 64);
        assert_eq!(builtin_llama_max_tokens(64_000, 16_000), 64);
        assert_eq!(builtin_llama_max_tokens(960_000, 16_000), 512);
        assert_eq!(builtin_llama_max_tokens(1_920_000, 16_000), 992);
        assert_eq!(builtin_llama_max_tokens(19_200_000, 16_000), 9632);
        assert_eq!(builtin_llama_max_tokens(2, 0), 48);
    }

    #[tokio::test]
    async fn builtin_llama_sends_temperature_zero_and_max_tokens_while_external_omits_them() {
        let (url, seen) = recording_server(r#"{"text":"language Cantonese<asr_text>你好"}"#).await;

        let mut builtin = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: url.clone(),
            model: "qwen3-asr-1.7b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        });
        builtin.connect(&SttConfig::default()).await.unwrap();
        builtin.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(builtin.disconnect().await.unwrap().as_deref(), Some("你好"));
        let builtin_req = String::from_utf8_lossy(&seen.lock().unwrap()).to_string();
        assert!(builtin_req.contains("name=\"temperature\"\r\n\r\n0\r\n"));
        assert!(builtin_req.contains("name=\"max_tokens\"\r\n\r\n64\r\n"));

        let mut external = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "external-whisper".to_string(),
            endpoint: url,
            model: "whisper-1".to_string(),
            verbose_json: false,
            builtin_llama_server: false,
        });
        external.connect(&SttConfig::default()).await.unwrap();
        external.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            external.disconnect().await.unwrap().as_deref(),
            Some("你好")
        );
        let external_req = String::from_utf8_lossy(&seen.lock().unwrap()).to_string();
        assert!(!external_req.contains("name=\"temperature\""));
        assert!(!external_req.contains("name=\"max_tokens\""));
    }

    #[tokio::test]
    async fn builtin_llama_drops_truncated_prompt_echo_while_external_is_unaffected() {
        let dictionary = vec![
            "OraType".to_string(),
            "Sennett".to_string(),
            "鰂魚涌".to_string(),
            "太古坊".to_string(),
        ];
        let (echo_url, _) = recording_server(
            r#"{"text":"language Cantonese<asr_text>OraType, Sennett, 鰂魚","finish_reason":"length"}"#,
        ).await;

        let mut builtin = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: echo_url.clone(),
            model: "qwen3-asr-1.7b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        });
        builtin
            .connect(&SttConfig {
                vocabulary: dictionary.clone(),
                ..SttConfig::default()
            })
            .await
            .unwrap();
        builtin.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(builtin.disconnect().await.unwrap(), None);

        let mut external = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "external-whisper".to_string(),
            endpoint: echo_url,
            model: "whisper-1".to_string(),
            verbose_json: false,
            builtin_llama_server: false,
        });
        external
            .connect(&SttConfig {
                vocabulary: dictionary.clone(),
                ..SttConfig::default()
            })
            .await
            .unwrap();
        external.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            external.disconnect().await.unwrap().as_deref(),
            Some("OraType, Sennett, 鰂魚")
        );

        let (completed_url, _) = recording_server(
            r#"{"text":"language Cantonese<asr_text>OraType, Sennett, 鰂魚","finish_reason":"stop"}"#,
        ).await;
        builtin.provider_config.endpoint = completed_url;
        builtin
            .connect(&SttConfig {
                vocabulary: dictionary.clone(),
                ..SttConfig::default()
            })
            .await
            .unwrap();
        builtin.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            builtin.disconnect().await.unwrap().as_deref(),
            Some("OraType, Sennett, 鰂魚")
        );

        let (speech_url, _) = recording_server(
            r#"{"text":"language Cantonese<asr_text>我陣間用 OraType 去鰂魚涌開會。","finish_reason":"length"}"#,
        )
        .await;
        let mut builtin_speech = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: speech_url,
            model: "qwen3-asr-1.7b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        });
        builtin_speech
            .connect(&SttConfig {
                vocabulary: dictionary,
                ..SttConfig::default()
            })
            .await
            .unwrap();
        builtin_speech.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            builtin_speech.disconnect().await.unwrap().as_deref(),
            Some("我陣間用 OraType 去鰂魚涌開會。")
        );
    }

    #[tokio::test]
    async fn builtin_llama_fails_fast_on_timeout_but_retries_5xx_while_external_retries_timeout() {
        let slow_delay = std::time::Duration::from_millis(400);
        let fast_timeout = std::time::Duration::from_millis(80);

        // 1. Built-in llama-server fails fast on timeout (1 attempt, no retry).
        let (timeout_url, timeout_hits) =
            scripted_server(move |_| (slow_delay, 200, r#"{"text":"late"}"#.to_string())).await;
        let mut builtin_timeout = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: timeout_url,
            model: "qwen3-asr-0.6b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        })
        .with_request_timeout(fast_timeout)
        .with_retry_backoff_ms(10);
        builtin_timeout
            .connect(&SttConfig::default())
            .await
            .unwrap();
        builtin_timeout.send_audio(&test_audio_2s()).await.unwrap();
        assert!(builtin_timeout.disconnect().await.is_err());
        assert_eq!(timeout_hits.load(std::sync::atomic::Ordering::SeqCst), 1);

        // 2. Built-in llama-server still retries HTTP 5xx up to 3 total attempts.
        let (retry_5xx_url, retry_5xx_hits) = scripted_server(|attempt| {
            if attempt < 3 {
                (
                    std::time::Duration::ZERO,
                    503,
                    r#"{"error":"busy"}"#.to_string(),
                )
            } else {
                (
                    std::time::Duration::ZERO,
                    200,
                    r#"{"text":"language Cantonese<asr_text>得咗"}"#.to_string(),
                )
            }
        })
        .await;
        let mut builtin_5xx = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: retry_5xx_url,
            model: "qwen3-asr-0.6b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        })
        .with_request_timeout(fast_timeout)
        .with_retry_backoff_ms(10);
        builtin_5xx.connect(&SttConfig::default()).await.unwrap();
        builtin_5xx.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            builtin_5xx.disconnect().await.unwrap().as_deref(),
            Some("得咗")
        );
        assert_eq!(retry_5xx_hits.load(std::sync::atomic::Ordering::SeqCst), 3);

        // 3. Built-in llama-server fails promptly on connect errors.
        let mut builtin_connect = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: "http://127.0.0.1:9/v1/audio/transcriptions".to_string(),
            model: "qwen3-asr-0.6b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        })
        .with_request_timeout(fast_timeout)
        .with_retry_backoff_ms(10);
        builtin_connect
            .connect(&SttConfig::default())
            .await
            .unwrap();
        builtin_connect.send_audio(&test_audio_2s()).await.unwrap();
        assert!(builtin_connect.disconnect().await.is_err());

        // 4. External server preserves existing timeout retries (3 total attempts).
        let (ext_timeout_url, ext_hits) = scripted_server(move |attempt| {
            if attempt < 3 {
                (slow_delay, 200, r#"{"text":"late"}"#.to_string())
            } else {
                (
                    std::time::Duration::ZERO,
                    200,
                    r#"{"text":"Recovered"}"#.to_string(),
                )
            }
        })
        .await;
        let mut external_timeout = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "external-whisper".to_string(),
            endpoint: ext_timeout_url,
            model: "whisper-1".to_string(),
            verbose_json: false,
            builtin_llama_server: false,
        })
        .with_request_timeout(fast_timeout)
        .with_retry_backoff_ms(10);
        external_timeout
            .connect(&SttConfig::default())
            .await
            .unwrap();
        external_timeout.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            external_timeout.disconnect().await.unwrap().as_deref(),
            Some("Recovered")
        );
        assert_eq!(ext_hits.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn builtin_llama_logs_cap_hit_metrics_privately_and_keeps_transcript() {
        use tracing::instrument::WithSubscriber as _;

        // Unit checks on `detect_output_cap_hit`.
        assert_eq!(
            detect_output_cap_hit(&serde_json::json!({"usage": {"output_tokens": 64}}), 64),
            Some(CapHitMetrics {
                max_tokens: 64,
                output_tokens: Some(64),
                finish_reason_length: false,
            })
        );
        assert_eq!(
            detect_output_cap_hit(&serde_json::json!({"usage": {"completion_tokens": 65}}), 64),
            Some(CapHitMetrics {
                max_tokens: 64,
                output_tokens: Some(65),
                finish_reason_length: false,
            })
        );
        assert_eq!(
            detect_output_cap_hit(&serde_json::json!({"finish_reason": "length"}), 64),
            Some(CapHitMetrics {
                max_tokens: 64,
                output_tokens: None,
                finish_reason_length: true,
            })
        );
        assert_eq!(
            detect_output_cap_hit(
                &serde_json::json!({"choices": [{"finish_reason": "length"}]}),
                64
            ),
            Some(CapHitMetrics {
                max_tokens: 64,
                output_tokens: None,
                finish_reason_length: true,
            })
        );
        assert_eq!(
            detect_output_cap_hit(
                &serde_json::json!({"usage": {"output_tokens": 63}, "finish_reason": "stop"}),
                64
            ),
            None
        );

        let transcript = "呢句係正常轉寫內容";
        let payload = serde_json::json!({
            "text": format!("language Cantonese<asr_text>{transcript}"),
            "usage": {
                "type": "tokens",
                "input_tokens": 30,
                "output_tokens": 64,
                "total_tokens": 94
            }
        })
        .to_string();
        let (url, _) = recording_server(payload).await;

        let buffer = TestLogBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();

        async {
            let mut builtin = WhisperCompatProvider::new(WhisperCompatConfig {
                provider_name: "builtin-qwen".to_string(),
                endpoint: url.clone(),
                model: "qwen3-asr-1.7b".to_string(),
                verbose_json: false,
                builtin_llama_server: true,
            });
            builtin.connect(&SttConfig::default()).await.unwrap();
            builtin.send_audio(&test_audio_2s()).await.unwrap();
            assert_eq!(
                builtin.disconnect().await.unwrap().as_deref(),
                Some(transcript)
            );
        }
        .with_subscriber(subscriber)
        .await;

        let builtin_logs = buffer.text();
        assert!(builtin_logs.contains("hit_cap=true"), "{builtin_logs}");
        assert!(builtin_logs.contains("max_tokens=64"), "{builtin_logs}");
        assert!(
            builtin_logs.contains("output_tokens=Some(64)"),
            "{builtin_logs}"
        );
        assert!(!builtin_logs.contains(transcript), "{builtin_logs}");

        let ext_buffer = TestLogBuffer::default();
        let ext_writer = ext_buffer.clone();
        let ext_subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(move || ext_writer.clone())
            .finish();

        async {
            let mut external = WhisperCompatProvider::new(WhisperCompatConfig {
                provider_name: "external-whisper".to_string(),
                endpoint: url,
                model: "whisper-1".to_string(),
                verbose_json: false,
                builtin_llama_server: false,
            });
            external.connect(&SttConfig::default()).await.unwrap();
            external.send_audio(&test_audio_2s()).await.unwrap();
            assert_eq!(
                external.disconnect().await.unwrap().as_deref(),
                Some(transcript)
            );
        }
        .with_subscriber(ext_subscriber)
        .await;

        assert!(!ext_buffer.text().contains("hit_cap=true"));
    }

    #[tokio::test]
    async fn default_llama_server_prompt_echo_is_only_dropped_for_builtin_on_short_audio() {
        let prompt_payload = serde_json::json!({
            "text": "language English<asr_text>Transcribe audio to text"
        })
        .to_string();
        let (prompt_url, _) = recording_server(prompt_payload).await;
        let mut provider = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: prompt_url,
            model: "qwen3-asr-1.7b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        });
        provider.connect(&SttConfig::default()).await.unwrap();
        provider.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(provider.disconnect().await.unwrap(), None);

        provider.provider_config.builtin_llama_server = false;
        provider.connect(&SttConfig::default()).await.unwrap();
        provider.send_audio(&test_audio_2s()).await.unwrap();
        assert_eq!(
            provider.disconnect().await.unwrap().as_deref(),
            Some("Transcribe audio to text")
        );
    }

    #[test]
    fn builtin_llama_request_timeout_scales_with_duration_while_external_stays_60s() {
        let builtin = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: "http://127.0.0.1:4321/v1/audio/transcriptions".to_string(),
            model: "qwen3-asr-1.7b".to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        });
        let external = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "external-whisper".to_string(),
            endpoint: "https://example.test/v1/audio/transcriptions".to_string(),
            model: "whisper-1".to_string(),
            verbose_json: true,
            builtin_llama_server: false,
        });

        let small = WhisperCompatProvider::new(WhisperCompatConfig {
            provider_name: "builtin-qwen".to_string(),
            endpoint: builtin.provider_config.endpoint.clone(),
            model: super::super::models::QWEN3_ASR_SMALL_MODEL_ID.to_string(),
            verbose_json: false,
            builtin_llama_server: true,
        });
        assert!(small.uses_scaled_timeout(400.0));
        assert_eq!(small.request_timeout_for(400.0).as_secs(), 630);
        assert!(!small.uses_scaled_timeout(400.1));
        assert_eq!(small.request_timeout_for(600.0).as_secs(), 60);

        for (dur_s, expected_secs) in [
            (0.0, 60),
            (10.0, 60),
            (20.0, 60),
            (20.1, 61),
            (60.0, 120),
            (180.0, 300),
            (600.0, 930),
            (720.0, 1110),
            (-1.0, 60),
            (-600.0, 60),
            (f64::NAN, 60),
            (f64::INFINITY, 60),
            (f64::NEG_INFINITY, 60),
            (f64::MAX, 60),
        ] {
            let expected = std::time::Duration::from_secs(expected_secs);
            assert_eq!(
                builtin_llama_request_timeout(dur_s),
                expected,
                "dur_s={dur_s:?}"
            );
            assert_eq!(
                builtin.request_timeout_for(dur_s),
                expected,
                "builtin provider dur_s={dur_s:?}"
            );
            assert_eq!(
                external.request_timeout_for(dur_s),
                std::time::Duration::from_secs(60),
                "external provider must stay at 60s for dur_s={dur_s:?}"
            );
        }
    }
}
