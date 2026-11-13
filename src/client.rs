use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use subtle::ConstantTimeEq;

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    /// Accept self-signed or otherwise invalid TLS certificates. The CM3500B
    /// ships a self-signed certificate and offers no certificate management,
    /// so this is required for HTTPS unless a fingerprint is pinned.
    pub insecure_tls: bool,
    /// Optional PEM CA certificate to trust instead of disabling verification.
    pub ca_cert: Option<PathBuf>,
    /// Optional SHA-256 fingerprint of the modem's leaf certificate. Pinning
    /// preserves authentication without depending on a CA or hostname match.
    pub cert_fingerprint: Option<[u8; 32]>,
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
    pub dhcp_log: String,
    pub dsx: String,
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

        if let Some(fingerprint) = options.cert_fingerprint {
            tracing::info!(
                "Pinning modem TLS certificate SHA-256 {}",
                format_fingerprint(&fingerprint)
            );
            builder = builder.use_preconfigured_tls(pinned_tls_config(fingerprint)?);
        } else if options.insecure_tls {
            tracing::warn!(
                "TLS certificate verification is disabled (--insecure-tls); \
                 modem credentials could be intercepted on an untrusted network. \
                 Prefer --tls-cert-fingerprint for the modem's fixed certificate."
            );
            builder = builder.use_preconfigured_tls(insecure_tls_config()?);
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

        let status = resp.status();
        if !status.is_success() && !status.is_redirection() {
            return Err(anyhow!("Login failed: HTTP {}", status));
        }

        let body = resp.text().await?;
        if body.contains("csrf_token") || status.is_redirection() {
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
        if Self::is_session_expired(&html) {
            return Err(anyhow!("Session expired"));
        }
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

        let html = resp.text().await?;
        if Self::is_session_expired(&html) {
            return Err(anyhow!("Session expired"));
        }
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

    async fn fetch_optional(&self, endpoint: &str) -> Result<String> {
        match self.fetch_page(endpoint).await {
            Ok(html) => Ok(html),
            Err(e) if e.to_string().contains("Session expired") => Err(e),
            Err(e) => {
                tracing::warn!("Optional page {} failed ({}), continuing", endpoint, e);
                Ok(String::new())
            }
        }
    }

    async fn try_fetch_all(&self) -> Result<ScrapedPages> {
        let status = self.fetch_page("status_cgi").await?;
        let vers = self.fetch_page("vers_cgi").await?;
        let dhcp = self.fetch_optional("dhcp_cgi").await?;
        let qos = self.fetch_optional("qos_cgi").await?;
        let cm_state = self.fetch_optional("cm_state_cgi").await?;
        let event = self.fetch_optional("event_cgi").await?;
        let config_params = self.fetch_optional("config_params_cgi").await?;
        let product = self.fetch_optional("product_cgi").await?;
        let dhcp_log = self.fetch_optional("dhcp_cmLog_cgi").await?;
        let dsx = self.fetch_optional("dsx_cgi").await?;

        Ok(ScrapedPages {
            status,
            vers,
            dhcp,
            qos,
            cm_state,
            event,
            config_params,
            product,
            dhcp_log,
            dsx,
        })
    }
}

/// Parse a SHA-256 certificate fingerprint in `SHA256:AA:BB:...`, colon-separated,
/// or compact hex form. Case and separators are ignored.
pub fn parse_fingerprint(input: &str) -> Result<[u8; 32]> {
    let trimmed = input.trim();
    let without_prefix = trimmed
        .strip_prefix("SHA256:")
        .or_else(|| trimmed.strip_prefix("sha256:"))
        .unwrap_or(trimmed);
    let hex: String = without_prefix
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | ' '))
        .collect();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "certificate fingerprint must be 64 hex characters (SHA-256), got {}",
            hex.len()
        ));
    }

    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .context("invalid certificate fingerprint")?;
    }
    Ok(out)
}

fn format_fingerprint(fingerprint: &[u8; 32]) -> String {
    fingerprint
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join(":")
}

fn pinned_tls_config(fingerprint: [u8; 32]) -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(FingerprintVerifier {
        expected: fingerprint,
        provider: provider.clone(),
    });
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("failed to configure TLS protocol versions")?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

fn insecure_tls_config() -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(InsecureVerifier {
        provider: provider.clone(),
    });
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("failed to configure TLS protocol versions")?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

#[derive(Debug)]
struct InsecureVerifier {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for InsecureVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[derive(Debug)]
struct FingerprintVerifier {
    expected: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let actual = ring::digest::digest(&ring::digest::SHA256, end_entity.as_ref());
        if actual.as_ref().ct_eq(&self.expected).into() {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "modem TLS certificate fingerprint mismatch".into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fingerprint_formats() {
        let expected = [
            0x25, 0x05, 0x2e, 0x73, 0x08, 0xc7, 0xa1, 0x9b, 0xa7, 0xa1, 0xd1, 0xca, 0x73, 0x22,
            0x7f, 0x01, 0x12, 0xdf, 0xbb, 0x25, 0x16, 0x69, 0x65, 0x07, 0xa2, 0x19, 0xf8, 0x3e,
            0x2a, 0x1b, 0x11, 0x79,
        ];
        let colon = "SHA256:25:05:2E:73:08:C7:A1:9B:A7:A1:D1:CA:73:22:7F:01:12:DF:BB:25:16:69:65:07:A2:19:F8:3E:2A:1B:11:79";
        let compact = "25052e7308c7a19ba7a1d1ca73227f0112dfbb2516696507a219f83e2a1b1179";
        assert_eq!(parse_fingerprint(colon).unwrap(), expected);
        assert_eq!(parse_fingerprint(compact).unwrap(), expected);
    }

    #[test]
    fn rejects_malformed_fingerprints() {
        assert!(parse_fingerprint("not-a-fingerprint").is_err());
        assert!(parse_fingerprint(&"aa".repeat(31)).is_err());
        assert!(parse_fingerprint(&"zz".repeat(32)).is_err());
    }
}
