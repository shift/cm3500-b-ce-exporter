use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    /// Accept self-signed or otherwise invalid TLS certificates. The CM3500B
    /// ships a self-signed certificate and offers no certificate management,
    /// so this is required for HTTPS in most deployments.
    pub insecure_tls: bool,
    /// Optional PEM CA certificate to trust instead of disabling verification.
    pub ca_cert: Option<PathBuf>,
}

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
    pub product: String,
}

impl ModemClient {
    pub fn new(
        base_url: &str,
        username: &str,
        password: &str,
        options: &ClientOptions,
    ) -> Result<Self> {
        let mut builder = Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(options.connect_timeout)
            .timeout(options.request_timeout);

        if options.insecure_tls {
            tracing::warn!(
                "TLS certificate verification is disabled (--insecure-tls); \
                 modem credentials could be intercepted on an untrusted network"
            );
            builder = builder.danger_accept_invalid_certs(true);
        } else if let Some(path) = &options.ca_cert {
            let pem = std::fs::read(path)
                .with_context(|| format!("failed to read CA certificate {}", path.display()))?;
            let cert = reqwest::Certificate::from_pem(&pem)
                .with_context(|| format!("invalid PEM CA certificate {}", path.display()))?;
            builder = builder.add_root_certificate(cert);
        }

        let client = builder.build()?;
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

    pub async fn post_page(&self, endpoint: &str, form: &[(&str, &str)]) -> Result<String> {
        let resp = self
            .client
            .post(format!("{}/cgi-bin/{}", self.base_url, endpoint))
            .form(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("POST {} failed: HTTP {}", endpoint, resp.status()));
        }

        Ok(resp.text().await?)
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

    pub async fn fetch_spectrum(&self) -> Result<String> {
        let result = self
            .post_page(
                "spectrum_cgi",
                &[("scan", "Scan"), ("centerSeq", "500"), ("widthSeq", "1000")],
            )
            .await;
        match result {
            Ok(html) => Ok(html),
            Err(e) => {
                tracing::warn!("Spectrum fetch failed ({}), re-authenticating...", e);
                self.login().await?;
                self.post_page(
                    "spectrum_cgi",
                    &[("scan", "Scan"), ("centerSeq", "500"), ("widthSeq", "1000")],
                )
                .await
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
        let product = self.fetch_page("product_cgi").await?;

        Ok(ScrapedPages {
            status,
            vers,
            dhcp,
            qos,
            cm_state,
            event,
            config_params,
            product,
        })
    }
}
