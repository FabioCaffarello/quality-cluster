use serde_json::Value;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// HTTP client wrapper for quality-service API calls.
pub struct ApiClient {
    base_url: String,
    correlation_id: String,
}

impl ApiClient {
    pub fn new(base_url: &str, run_id: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            correlation_id: format!("raccoon-smoke-{run_id}"),
        }
    }

    /// GET /healthz
    pub fn healthz(&self) -> Result<u16, String> {
        let url = format!("{}/healthz", self.base_url);
        match ureq::get(&url).timeout(REQUEST_TIMEOUT).call() {
            Ok(resp) => Ok(resp.status()),
            Err(ureq::Error::Status(code, _)) => Ok(code),
            Err(e) => Err(format!("healthz request failed: {e}")),
        }
    }

    /// GET /readyz
    pub fn readyz(&self) -> Result<u16, String> {
        let url = format!("{}/readyz", self.base_url);
        match ureq::get(&url).timeout(REQUEST_TIMEOUT).call() {
            Ok(resp) => Ok(resp.status()),
            Err(ureq::Error::Status(code, _)) => Ok(code),
            Err(e) => Err(format!("readyz request failed: {e}")),
        }
    }

    /// POST /configctl/configs — create a draft config.
    /// Returns the parsed JSON response body.
    pub fn create_draft(&self, name: &str, content: &Value) -> Result<Value, String> {
        let url = format!("{}/configctl/configs", self.base_url);
        let content_str = serde_json::to_string(content)
            .map_err(|e| format!("failed to serialize config content: {e}"))?;
        let body = serde_json::json!({
            "name": name,
            "format": "json",
            "content": content_str
        });
        self.post_json(&url, &body)
    }

    /// POST /configctl/config-versions/:id/validate
    pub fn validate_config(&self, id: &str) -> Result<Value, String> {
        let url = format!(
            "{}/configctl/config-versions/{}/validate",
            self.base_url, id
        );
        self.post_json(&url, &serde_json::json!({}))
    }

    /// POST /configctl/config-versions/:id/compile
    pub fn compile_config(&self, id: &str) -> Result<Value, String> {
        let url = format!("{}/configctl/config-versions/{}/compile", self.base_url, id);
        self.post_json(&url, &serde_json::json!({}))
    }

    /// POST /configctl/config-versions/:id/activate
    pub fn activate_config(
        &self,
        id: &str,
        scope_kind: &str,
        scope_key: &str,
    ) -> Result<Value, String> {
        let url = format!(
            "{}/configctl/config-versions/{}/activate",
            self.base_url, id
        );
        let body = serde_json::json!({
            "scope_kind": scope_kind,
            "scope_key": scope_key
        });
        self.post_json(&url, &body)
    }

    /// GET /runtime/ingestion/bindings
    pub fn ingestion_bindings(&self, scope_kind: &str, scope_key: &str) -> Result<Value, String> {
        self.ingestion_bindings_scoped(scope_kind, scope_key)
    }

    /// GET /runtime/validator/results
    pub fn validation_results(
        &self,
        scope_kind: &str,
        scope_key: &str,
        limit: u32,
    ) -> Result<Value, String> {
        self.validation_results_scoped(scope_kind, scope_key, limit)
    }

    /// GET /runtime/validator/results with custom scope
    pub fn validation_results_scoped(
        &self,
        scope_kind: &str,
        scope_key: &str,
        limit: u32,
    ) -> Result<Value, String> {
        let url = format!(
            "{}/runtime/validator/results?scope_kind={}&scope_key={}&limit={}",
            self.base_url, scope_kind, scope_key, limit
        );
        self.get_json(&url)
    }

    /// GET /runtime/ingestion/bindings with custom scope
    pub fn ingestion_bindings_scoped(
        &self,
        scope_kind: &str,
        scope_key: &str,
    ) -> Result<Value, String> {
        let url = format!(
            "{}/runtime/ingestion/bindings?scope_kind={}&scope_key={}",
            self.base_url, scope_kind, scope_key
        );
        self.get_json(&url)
    }

    /// GET /configctl/configs/active
    pub fn get_active_config(&self, scope_kind: &str, scope_key: &str) -> Result<Value, String> {
        let url = format!(
            "{}/configctl/configs/active?scope_kind={}&scope_key={}",
            self.base_url, scope_kind, scope_key
        );
        self.get_json(&url)
    }

    /// GET /runtime/configctl/projections
    pub fn configctl_runtime_projections(
        &self,
        scope_kind: &str,
        scope_key: &str,
    ) -> Result<Value, String> {
        let url = format!(
            "{}/runtime/configctl/projections?scope_kind={}&scope_key={}",
            self.base_url, scope_kind, scope_key
        );
        self.get_json(&url)
    }

    /// GET /runtime/validator/active
    pub fn validator_runtime(&self, scope_kind: &str, scope_key: &str) -> Result<Value, String> {
        let url = format!(
            "{}/runtime/validator/active?scope_kind={}&scope_key={}",
            self.base_url, scope_kind, scope_key
        );
        self.get_json(&url)
    }

    fn post_json(&self, url: &str, body: &Value) -> Result<Value, String> {
        let resp = ureq::post(url)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .set("X-Correlation-ID", &self.correlation_id)
            .timeout(REQUEST_TIMEOUT)
            .send_json(body.clone())
            .map_err(|e| format!("POST {url} failed: {e}"))?;

        resp.into_json::<Value>()
            .map_err(|e| format!("failed to parse response from {url}: {e}"))
    }

    fn get_json(&self, url: &str) -> Result<Value, String> {
        let resp = ureq::get(url)
            .set("Accept", "application/json")
            .set("X-Correlation-ID", &self.correlation_id)
            .timeout(REQUEST_TIMEOUT)
            .call()
            .map_err(|e| format!("GET {url} failed: {e}"))?;

        resp.into_json::<Value>()
            .map_err(|e| format!("failed to parse response from {url}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_client_strips_trailing_slash() {
        let client = ApiClient::new("http://localhost:8080/", "run-1");
        assert_eq!(client.base_url, "http://localhost:8080");
    }

    #[test]
    fn api_client_correlation_id_contains_pid() {
        let client = ApiClient::new("http://localhost:8080", "run-123");
        assert_eq!(client.correlation_id, "raccoon-smoke-run-123");
    }
}
