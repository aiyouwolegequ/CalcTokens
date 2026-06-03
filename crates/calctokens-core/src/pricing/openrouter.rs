use super::{cache, ModelPricing};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;

const CACHE_FILENAME: &str = "pricing-openrouter.json";
const MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_CONCURRENT_REQUESTS: usize = 10;
const MAX_MODELS_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENDPOINT_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MODELS_PER_SYNC: usize = 5000;
const MAX_ENDPOINTS_PER_MODEL: usize = 512;

/// Structs for `/api/v1/models` endpoint (list all models).

#[derive(Deserialize)]
struct ModelListPricing {
    prompt: String,
    completion: String,
}

#[derive(Deserialize)]
struct ModelListItem {
    id: String,
    pricing: Option<ModelListPricing>,
}

#[derive(Deserialize)]
struct ModelsListResponse {
    data: Vec<ModelListItem>,
}

/// Structs for `/api/v1/models/{id}/endpoints` endpoint (author pricing).

#[derive(Deserialize)]
struct EndpointPricing {
    prompt: String,
    completion: String,
    #[serde(default)]
    input_cache_read: Option<String>,
    #[serde(default)]
    input_cache_write: Option<String>,
}

#[derive(Deserialize)]
struct Endpoint {
    provider_name: String,
    pricing: EndpointPricing,
}

#[derive(Deserialize)]
struct EndpointData {
    #[allow(dead_code)]
    id: String,
    endpoints: Vec<Endpoint>,
}

#[derive(Deserialize)]
struct EndpointsResponse {
    data: EndpointData,
}

/// Model ID prefix to provider name mapping.
///
/// Translates model ID prefixes like `z-ai` to their corresponding
/// provider names in the endpoints API, such as `Z.AI`.
fn get_author_provider_name(model_id: &str) -> Option<&'static str> {
    let prefix = model_id.split('/').next()?;

    match prefix.to_lowercase().as_str() {
        "z-ai" => Some("Z.AI"),
        "x-ai" => Some("xAI"),
        "anthropic" => Some("Anthropic"),
        "openai" => Some("OpenAI"),
        "google" => Some("Google"),
        "meta-llama" => Some("Meta"),
        "mistralai" => Some("Mistral"),
        "deepseek" => Some("DeepSeek"),
        "qwen" => Some("Alibaba"),
        "cohere" => Some("Cohere"),
        "perplexity" => Some("Perplexity"),
        "moonshotai" => Some("Moonshot AI"),
        _ => None,
    }
}

pub fn load_cached() -> Option<HashMap<String, ModelPricing>> {
    cache::load_cache(CACHE_FILENAME)
}

pub fn load_cached_any_age() -> Option<HashMap<String, ModelPricing>> {
    cache::load_cache_any_age(CACHE_FILENAME)
}

fn parse_price(s: &str) -> Option<f64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

async fn read_response_limited(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(format!("response body exceeded {} bytes", max_bytes));
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("failed to read response body: {}", err))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| "response body size overflow".to_string())?;
        if next_len > max_bytes {
            return Err(format!("response body exceeded {} bytes", max_bytes));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_json_bytes_limited<T: DeserializeOwned>(
    body: &[u8],
    max_bytes: usize,
) -> Result<T, String> {
    if body.len() > max_bytes {
        return Err(format!("response body exceeded {} bytes", max_bytes));
    }
    serde_json::from_slice(body).map_err(|err| format!("invalid JSON response: {}", err))
}

async fn parse_json_response_limited<T: DeserializeOwned>(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<T, String> {
    parse_json_bytes_limited(
        &read_response_limited(response, max_bytes).await?,
        max_bytes,
    )
}

fn collect_model_fallbacks(
    data: ModelsListResponse,
) -> Result<Vec<(String, Option<ModelPricing>)>, String> {
    if data.data.len() > MAX_MODELS_PER_SYNC {
        return Err(format!(
            "OpenRouter returned {} models, exceeding the limit of {}",
            data.data.len(),
            MAX_MODELS_PER_SYNC
        ));
    }

    Ok(data
        .data
        .into_iter()
        .map(|m| {
            let fallback = m.pricing.and_then(|p| {
                let input = parse_price(&p.prompt)?;
                let output = parse_price(&p.completion)?;
                Some(ModelPricing {
                    input_cost_per_token: Some(input),
                    output_cost_per_token: Some(output),
                    cache_read_input_token_cost: None,
                    cache_creation_input_token_cost: None,
                    ..Default::default()
                })
            });
            (m.id, fallback)
        })
        .collect())
}

async fn fetch_author_pricing(
    client: Arc<reqwest::Client>,
    model_id: String,
    semaphore: Arc<Semaphore>,
    fallback_pricing: Option<ModelPricing>,
) -> Option<(String, ModelPricing)> {
    let _permit = semaphore.acquire().await.ok()?;

    let author_name = match get_author_provider_name(&model_id) {
        Some(name) => name,
        None => return fallback_pricing.map(|p| (model_id, p)),
    };

    let url = format!("https://openrouter.ai/api/v1/models/{}/endpoints", model_id);

    let response = match client
        .get(&url)
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return fallback_pricing.map(|p| (model_id, p));
        }
    };

    if !response.status().is_success() {
        return fallback_pricing.map(|p| (model_id, p));
    }

    let data: EndpointsResponse =
        match parse_json_response_limited(response, MAX_ENDPOINT_RESPONSE_BYTES).await {
            Ok(d) => d,
            Err(_) => {
                return fallback_pricing.map(|p| (model_id, p));
            }
        };

    if data.data.endpoints.len() > MAX_ENDPOINTS_PER_MODEL {
        return fallback_pricing.map(|p| (model_id, p));
    }

    // Find the endpoint from the author provider
    let author_endpoint = match data
        .data
        .endpoints
        .iter()
        .find(|e| e.provider_name == author_name)
    {
        Some(ep) => ep,
        None => {
            return fallback_pricing.map(|p| (model_id, p));
        }
    };

    let input_cost = parse_price(&author_endpoint.pricing.prompt);
    let output_cost = parse_price(&author_endpoint.pricing.completion);

    if input_cost.is_none() || output_cost.is_none() {
        return fallback_pricing.map(|p| (model_id, p));
    }

    let pricing = ModelPricing {
        input_cost_per_token: input_cost,
        output_cost_per_token: output_cost,
        cache_read_input_token_cost: author_endpoint
            .pricing
            .input_cache_read
            .as_ref()
            .and_then(|s| parse_price(s)),
        cache_creation_input_token_cost: author_endpoint
            .pricing
            .input_cache_write
            .as_ref()
            .and_then(|s| parse_price(s)),
        ..Default::default()
    };

    Some((model_id, pricing))
}

/// Fetch all models and get author pricing for each
pub async fn fetch_all_models() -> HashMap<String, ModelPricing> {
    if let Some(cached) = load_cached() {
        return cached;
    }

    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default(),
    );

    let mut last_error: Option<String> = None;

    let models_with_fallback: Vec<(String, Option<ModelPricing>)> = 'retry: {
        for attempt in 0..MAX_RETRIES {
            let response = match client
                .get(MODELS_URL)
                .header("Content-Type", "application/json")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("network error: {}", e));
                    if attempt < MAX_RETRIES - 1 {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            INITIAL_BACKOFF_MS * (1 << attempt),
                        ))
                        .await;
                    }
                    continue;
                }
            };

            let status = response.status();
            if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                last_error = Some(format!("HTTP {}", status));
                if attempt < MAX_RETRIES - 1 {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        INITIAL_BACKOFF_MS * (1 << attempt),
                    ))
                    .await;
                }
                continue;
            }

            if !status.is_success() {
                eprintln!("[calctokens] OpenRouter models API returned {}", status);
                break 'retry Vec::new();
            }

            let data: ModelsListResponse =
                match parse_json_response_limited(response, MAX_MODELS_RESPONSE_BYTES).await {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("[calctokens] OpenRouter models JSON parse failed: {}", e);
                        break 'retry Vec::new();
                    }
                };

            break 'retry match collect_model_fallbacks(data) {
                Ok(models) => models,
                Err(e) => {
                    eprintln!("[calctokens] {}", e);
                    Vec::new()
                }
            };
        }

        if let Some(err) = &last_error {
            eprintln!(
                "[calctokens] OpenRouter fetch failed after {} retries: {}",
                MAX_RETRIES, err
            );
        }
        Vec::new()
    };

    if models_with_fallback.is_empty() {
        return HashMap::new();
    }

    let models_with_authors: Vec<(String, Option<ModelPricing>)> = models_with_fallback
        .into_iter()
        .filter(|(id, _)| get_author_provider_name(id).is_some())
        .collect();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut result = HashMap::new();

    for batch in models_with_authors.chunks(MAX_CONCURRENT_REQUESTS) {
        let mut handles = Vec::with_capacity(batch.len());

        for (model_id, fallback) in batch {
            let client = Arc::clone(&client);
            let sem = Arc::clone(&semaphore);
            let model_id = model_id.clone();
            let fallback = fallback.clone();

            let handle =
                tokio::spawn(
                    async move { fetch_author_pricing(client, model_id, sem, fallback).await },
                );

            handles.push(handle);
        }

        for handle in handles {
            if let Ok(Some((model_id, pricing))) = handle.await {
                result.insert(model_id, pricing);
            }
        }
    }

    if !result.is_empty() {
        if let Err(e) = cache::save_cache(CACHE_FILENAME, &result) {
            eprintln!(
                "[calctokens] Warning: Failed to cache OpenRouter pricing at {}: {}",
                cache::get_cache_path(CACHE_FILENAME).display(),
                e
            );
        }
    }

    result
}

pub async fn fetch_all_mapped() -> HashMap<String, ModelPricing> {
    fetch_all_models().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_bytes_limited_rejects_oversized_models_body() {
        let body = br#"{"data":[]}"#;

        let err = match parse_json_bytes_limited::<ModelsListResponse>(body, 4) {
            Ok(_) => panic!("expected oversized body to be rejected"),
            Err(err) => err,
        };

        assert!(err.contains("response body exceeded"));
    }

    #[test]
    fn collect_model_fallbacks_rejects_excessive_model_count() {
        let data = ModelsListResponse {
            data: (0..=MAX_MODELS_PER_SYNC)
                .map(|i| ModelListItem {
                    id: format!("openai/model-{}", i),
                    pricing: None,
                })
                .collect(),
        };

        let err = collect_model_fallbacks(data).unwrap_err();

        assert!(err.contains("exceeding the limit"));
    }

    #[test]
    fn collect_model_fallbacks_preserves_valid_fallback_pricing() {
        let data = ModelsListResponse {
            data: vec![ModelListItem {
                id: "openai/gpt-test".to_string(),
                pricing: Some(ModelListPricing {
                    prompt: "0.000001".to_string(),
                    completion: "0.000002".to_string(),
                }),
            }],
        };

        let models = collect_model_fallbacks(data).unwrap();

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].0, "openai/gpt-test");
        let pricing = models[0].1.as_ref().unwrap();
        assert_eq!(pricing.input_cost_per_token, Some(0.000001));
        assert_eq!(pricing.output_cost_per_token, Some(0.000002));
    }
}
