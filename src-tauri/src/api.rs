use crate::models::*;
use reqwest::Client;

pub struct ApiClient {
    client: Client,
    base_url: String,
    token: String,
}

impl ApiClient {
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn get_me(&self) -> Result<UserResponse, String> {
        let resp = self
            .client
            .get(format!("{}/api/me", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {body}"));
        }

        resp.json().await.map_err(|e| format!("Parse error: {e}"))
    }

    pub async fn list_notifications(
        &self,
        status: Option<&str>,
        priority: Option<&str>,
        since: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<PaginatedNotifications, String> {
        let mut params = Vec::new();
        if let Some(s) = status {
            params.push(format!("status={s}"));
        }
        if let Some(p) = priority {
            params.push(format!("priority={p}"));
        }
        if let Some(s) = since {
            params.push(format!("since={s}"));
        }
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        if let Some(o) = offset {
            params.push(format!("offset={o}"));
        }

        let mut url = format!("{}/api/notifications/", self.base_url);
        if !params.is_empty() {
            url = format!("{url}?{}", params.join("&"));
        }

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {body}"));
        }

        resp.json().await.map_err(|e| format!("Parse error: {e}"))
    }

    pub async fn update_notification(
        &self,
        id: &str,
        status: &str,
    ) -> Result<Notification, String> {
        let resp = self
            .client
            .patch(format!("{}/api/notifications/{id}", self.base_url))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({ "status": status }))
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("API error {st}: {body}"));
        }

        resp.json().await.map_err(|e| format!("Parse error: {e}"))
    }

    pub async fn delete_notification(&self, id: &str) -> Result<(), String> {
        let resp = self
            .client
            .delete(format!("{}/api/notifications/{id}", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("API error {st}: {body}"));
        }

        Ok(())
    }
}
