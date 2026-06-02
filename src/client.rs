use anyhow::{anyhow, Result};
use reqwest::Client;

#[derive(Clone)]
pub struct ModemClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

pub struct ScrapedPages {
    pub status: String,
    pub vers: String,
    pub dhcp: String,
    pub qos: String,
    pub cm_state: String,
    pub event: String,
    pub config_params: String,
}

impl ModemClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let client = Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    pub async fn login(&self) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/cgi-bin/login_cgi", self.base_url))
            .form(&[
                ("username", self.username.as_str()),
                ("password", self.password.as_str()),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Login failed: HTTP {}", resp.status()));
        }

        let body = resp.text().await?;
        if body.contains("csrf_token") {
            tracing::info!("Login successful");
            Ok(())
        } else if body.contains("login_cgi") {
            Err(anyhow!("Login failed: credentials rejected"))
        } else {
            Err(anyhow!("Login failed: unexpected response"))
        }
    }

    pub async fn fetch_page(&self, endpoint: &str) -> Result<String> {
        let resp = self
            .client
            .get(format!("{}/cgi-bin/{}", self.base_url, endpoint))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Fetch {} failed: HTTP {}", endpoint, resp.status()));
        }

        let html = resp.text().await?;
        Ok(html)
    }

    fn is_session_expired(html: &str) -> bool {
        html.contains("url=login_cgi") || html.contains("Touchstone Login")
    }

    pub async fn fetch_all(&self) -> Result<ScrapedPages> {
        let pages = self.try_fetch_all().await;
        match pages {
            Ok(p) => Ok(p),
            Err(e) => {
                tracing::warn!("Fetch failed ({}), re-authenticating...", e);
                self.login().await?;
                self.try_fetch_all().await
            }
        }
    }

    async fn try_fetch_all(&self) -> Result<ScrapedPages> {
        let status = self.fetch_page("status_cgi").await?;
        if Self::is_session_expired(&status) {
            return Err(anyhow!("Session expired"));
        }

        let vers = self.fetch_page("vers_cgi").await?;
        let dhcp = self.fetch_page("dhcp_cgi").await?;
        let qos = self.fetch_page("qos_cgi").await?;
        let cm_state = self.fetch_page("cm_state_cgi").await?;
        let event = self.fetch_page("event_cgi").await?;
        let config_params = self.fetch_page("config_params_cgi").await?;

        Ok(ScrapedPages {
            status,
            vers,
            dhcp,
            qos,
            cm_state,
            event,
            config_params,
        })
    }
}
