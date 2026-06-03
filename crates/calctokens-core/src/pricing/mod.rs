pub mod aliases;
pub mod cache;
pub mod lookup;
pub mod openrouter;

use lookup::{LookupResult, PricingLookup};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::TokenBreakdown;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelPricing {
    pub input_cost_per_token: Option<f64>,
    pub input_cost_per_token_above_128k_tokens: Option<f64>,
    pub input_cost_per_token_above_200k_tokens: Option<f64>,
    pub input_cost_per_token_above_256k_tokens: Option<f64>,
    pub input_cost_per_token_above_272k_tokens: Option<f64>,
    pub output_cost_per_token: Option<f64>,
    pub output_cost_per_token_above_128k_tokens: Option<f64>,
    pub output_cost_per_token_above_200k_tokens: Option<f64>,
    pub output_cost_per_token_above_256k_tokens: Option<f64>,
    pub output_cost_per_token_above_272k_tokens: Option<f64>,
    pub cache_creation_input_token_cost: Option<f64>,
    pub cache_creation_input_token_cost_above_200k_tokens: Option<f64>,
    pub cache_read_input_token_cost: Option<f64>,
    pub cache_read_input_token_cost_above_200k_tokens: Option<f64>,
    pub cache_read_input_token_cost_above_272k_tokens: Option<f64>,
}

static PRICING_SERVICE: OnceCell<Arc<PricingService>> = OnceCell::const_new();

pub struct PricingService {
    lookup: PricingLookup,
}

impl PricingService {
    pub fn new(openrouter_data: HashMap<String, ModelPricing>) -> Self {
        Self {
            lookup: PricingLookup::new(HashMap::new(), openrouter_data, HashMap::new()),
        }
    }

    async fn fetch_inner() -> Result<Self, String> {
        let openrouter_data = openrouter::fetch_all_mapped().await;
        Ok(Self::new(openrouter_data))
    }

    fn from_cached_datasets(
        openrouter_data: Option<HashMap<String, ModelPricing>>,
    ) -> Option<Self> {
        openrouter_data.map(Self::new)
    }

    pub fn load_cached_any_age() -> Option<Self> {
        Self::from_cached_datasets(openrouter::load_cached_any_age())
    }

    pub async fn get_or_init() -> Result<Arc<PricingService>, String> {
        PRICING_SERVICE
            .get_or_try_init(|| async { Self::fetch_inner().await.map(Arc::new) })
            .await
            .map(Arc::clone)
    }

    pub fn lookup_with_source(
        &self,
        model_id: &str,
        force_source: Option<&str>,
    ) -> Option<LookupResult> {
        self.lookup.lookup_with_source(model_id, force_source)
    }

    pub fn lookup_with_source_and_provider(
        &self,
        model_id: &str,
        force_source: Option<&str>,
        provider_id: Option<&str>,
    ) -> Option<LookupResult> {
        self.lookup
            .lookup_with_source_and_provider(model_id, force_source, provider_id)
    }

    pub fn calculate_cost(
        &self,
        model_id: &str,
        input: i64,
        output: i64,
        cache_read: i64,
        cache_write: i64,
        reasoning: i64,
    ) -> f64 {
        let usage = TokenBreakdown {
            input,
            output,
            cache_read,
            cache_write,
            reasoning,
        };
        self.calculate_cost_with_provider(model_id, None, &usage)
    }

    pub fn calculate_cost_with_provider(
        &self,
        model_id: &str,
        provider_id: Option<&str>,
        usage: &TokenBreakdown,
    ) -> f64 {
        self.lookup
            .calculate_cost_with_provider(model_id, provider_id, usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_model_pricing_with_above_200k_fields() {
        let pricing: ModelPricing = serde_json::from_str(
            r#"{
                "input_cost_per_token": 0.0000015,
                "input_cost_per_token_above_200k_tokens": 0.000003,
                "output_cost_per_token": 0.0000075,
                "output_cost_per_token_above_200k_tokens": 0.000015,
                "cache_creation_input_token_cost": 0.000001875,
                "cache_creation_input_token_cost_above_200k_tokens": 0.00000375,
                "cache_read_input_token_cost": 0.00000015,
                "cache_read_input_token_cost_above_200k_tokens": 0.0000003
            }"#,
        )
        .unwrap();

        assert_eq!(pricing.input_cost_per_token, Some(0.0000015));
        assert_eq!(
            pricing.input_cost_per_token_above_200k_tokens,
            Some(0.000003)
        );
        assert_eq!(pricing.output_cost_per_token, Some(0.0000075));
        assert_eq!(
            pricing.output_cost_per_token_above_200k_tokens,
            Some(0.000015)
        );
        assert_eq!(pricing.cache_creation_input_token_cost, Some(0.000001875));
        assert_eq!(
            pricing.cache_creation_input_token_cost_above_200k_tokens,
            Some(0.00000375)
        );
        assert_eq!(pricing.cache_read_input_token_cost, Some(0.00000015));
        assert_eq!(
            pricing.cache_read_input_token_cost_above_200k_tokens,
            Some(0.0000003)
        );
    }

    #[test]
    fn deserialize_model_pricing_without_above_200k_fields() {
        let pricing: ModelPricing = serde_json::from_str(
            r#"{
                "input_cost_per_token": 0.00000125,
                "output_cost_per_token": 0.00001,
                "cache_creation_input_token_cost": 0.00000125,
                "cache_read_input_token_cost": 0.000000125
            }"#,
        )
        .unwrap();

        assert_eq!(pricing.input_cost_per_token, Some(0.00000125));
        assert_eq!(pricing.input_cost_per_token_above_200k_tokens, None);
        assert_eq!(pricing.output_cost_per_token, Some(0.00001));
        assert_eq!(pricing.output_cost_per_token_above_200k_tokens, None);
        assert_eq!(pricing.cache_creation_input_token_cost, Some(0.00000125));
        assert_eq!(
            pricing.cache_creation_input_token_cost_above_200k_tokens,
            None
        );
        assert_eq!(pricing.cache_read_input_token_cost, Some(0.000000125));
    }

    #[test]
    fn test_from_cached_datasets_returns_none_when_source_missing() {
        assert!(PricingService::from_cached_datasets(None).is_none());
    }
}
