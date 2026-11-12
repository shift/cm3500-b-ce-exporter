use cm3500_b_ce_exporter::{automation, client, debug_dump, metrics, otlp, parser};

use anyhow::{bail, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(
    name = "cm3500-exporter",
    about = "Prometheus exporter for ARRIS CM3500B CE cable modem",
    version
)]
struct Args {
    /// Modem base URL
    #[arg(long, default_value = "https://192.168.100.1")]
    modem_url: String,

    /// Modem login username
    #[arg(long, default_value = "admin")]
    username: String,

    /// Modem login password (or set MODEM_PASSWORD)
    #[arg(long, env = "MODEM_PASSWORD", default_value_t = String::new())]
    password: String,

    /// Listen address for the Prometheus metrics server
    #[arg(long, default_value = "127.0.0.1:10044")]
    listen: String,

    /// Disable the Prometheus /metrics HTTP endpoint entirely
    #[arg(long)]
    disable_prometheus: bool,

    /// Scrape interval in seconds
    #[arg(long, default_value_t = 30)]
    interval: u64,

    /// TCP connection timeout in seconds
    #[arg(long, default_value_t = 5)]
    connect_timeout: u64,

    /// Total HTTP request timeout in seconds
    #[arg(long, default_value_t = 15)]
    request_timeout: u64,

    /// Accept invalid/self-signed modem TLS certificates (required for the
    /// CM3500B's built-in certificate)
    #[arg(long)]
    insecure_tls: bool,

    /// PEM CA certificate to trust for modem TLS instead of --insecure-tls
    #[arg(long, value_name = "PATH")]
    ca_cert: Option<PathBuf>,

    /// SHA-256 fingerprint of the modem's TLS certificate. Pins the fixed
    /// ARRIS certificate without disabling authentication.
    #[arg(long, value_name = "SHA256")]
    tls_cert_fingerprint: Option<String>,

    /// On structural parse failure, write redacted raw modem HTML here
    #[arg(long, value_name = "DIR")]
    debug_html_dir: Option<PathBuf>,

    /// Mark /ready as failed when no successful scrape has completed within
    /// this many seconds [default: max(60, 3 × interval)]
    #[arg(long, value_name = "SECONDS")]
    readiness_stale_after: Option<u64>,

    /// Write connection state changes to this JSON file
    #[arg(long, value_name = "PATH")]
    state_file: Option<PathBuf>,

    /// Write configured upstream/downstream capacity to this JSON file
    #[arg(long, value_name = "PATH")]
    capacity_file: Option<PathBuf>,

    /// Consecutive bad scrapes or bad modem states required before reporting down
    #[arg(long, default_value_t = 3)]
    state_down_threshold: u32,

    /// Consecutive good scrapes required before reporting recovered
    #[arg(long, default_value_t = 2)]
    state_up_threshold: u32,

    /// Percent of configured service flow bandwidth to expose as shaped capacity
    #[arg(long, default_value_t = 95)]
    capacity_margin_percent: u8,

    /// Actively trigger a modem spectrum scan and export spectrum metrics
    #[arg(long)]
    enable_spectrum: bool,

    /// Interval between active spectrum scans in seconds
    #[arg(long, default_value_t = 900)]
    spectrum_interval: u64,

    /// OTLP HTTP base URL or /v1/metrics endpoint to push metrics and logs to
    #[arg(long)]
    otlp_endpoint: Option<String>,

    /// OTLP HTTP header in KEY=VALUE format (can be repeated)
    #[arg(long = "otlp-header", value_name = "KEY=VALUE")]
    otlp_headers: Vec<String>,

    /// Internal container healthcheck: fetch a URL and exit 0 on 2xx
    #[arg(long, hide = true, value_name = "URL")]
    check_ready: Option<String>,
}

struct ScrapeResult {
    metrics_text: String,
    observation: automation::ScrapeObservation,
    success: bool,
    error: Option<String>,
}

#[derive(Default)]
struct HealthState {
    last_success: Option<Instant>,
    last_error: Option<String>,
}

#[derive(Clone)]
struct AppState {
    metrics_text: Arc<RwLock<String>>,
    health: Arc<RwLock<HealthState>>,
    ready_stale_after: Duration,
}

#[derive(Default)]
struct SpectrumCache {
    html: Option<String>,
    last_scan_at: Option<Instant>,
}

impl SpectrumCache {
    fn should_refresh(&self, interval: Duration) -> bool {
        match self.last_scan_at {
            None => true,
            Some(last) => last.elapsed() >= interval,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cm3500_b_ce_exporter=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();
    if let Some(url) = args.check_ready.as_deref() {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(2))
            .build()?;
        let healthy = client
            .get(url)
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false);
        std::process::exit(if healthy { 0 } else { 1 });
    }
    validate_args(&args)?;

    if args.disable_prometheus && args.otlp_endpoint.is_none() {
        bail!("--disable-prometheus requires --otlp-endpoint");
    }

    let fingerprint_input = args
        .tls_cert_fingerprint
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let cert_fingerprint = fingerprint_input
        .map(client::parse_fingerprint)
        .transpose()?;
    if args.insecure_tls && cert_fingerprint.is_some() {
        tracing::warn!("--insecure-tls is ignored because --tls-cert-fingerprint is set");
    }
    let client_options = client::ClientOptions {
        connect_timeout: Duration::from_secs(args.connect_timeout),
        request_timeout: Duration::from_secs(args.request_timeout),
        insecure_tls: args.insecure_tls,
        ca_cert: args.ca_cert.clone(),
        cert_fingerprint,
    };
    let client = client::ModemClient::new(
        &args.modem_url,
        &args.username,
        &args.password,
        &client_options,
    )?;

    let otlp_headers: Vec<(String, String)> = args
        .otlp_headers
        .iter()
        .filter_map(|h| {
            let (k, v) = h.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect();

    let otlp_client: Arc<RwLock<Option<Arc<otlp::OtlpClient>>>> =
        if let Some(endpoint) = &args.otlp_endpoint {
            tracing::info!("OTLP push enabled, endpoint: {}", endpoint);
            Arc::new(RwLock::new(Some(Arc::new(otlp::OtlpClient::new_fallback(
                endpoint,
                otlp_headers,
                &args.modem_url,
            )))))
        } else {
            Arc::new(RwLock::new(None))
        };

    let ready_stale_after = Duration::from_secs(
        args.readiness_stale_after
            .unwrap_or_else(|| args.interval.saturating_mul(3).max(60)),
    );
    let state = AppState {
        metrics_text: Arc::new(RwLock::new(String::new())),
        health: Arc::new(RwLock::new(HealthState::default())),
        ready_stale_after,
    };

    // Bind and serve before the first modem scrape so /health and /ready are
    // available even while the modem is unreachable or rebooting.
    let server_handle = if args.disable_prometheus {
        tracing::info!("Prometheus HTTP endpoint disabled");
        None
    } else {
        let app = axum::Router::new()
            .route("/metrics", axum::routing::get(metrics_handler))
            .route("/health", axum::routing::get(health_handler))
            .route("/ready", axum::routing::get(ready_handler))
            .route("/", axum::routing::get(landing_handler))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind(&args.listen).await?;
        tracing::info!("Listening on {}", args.listen);
        Some(tokio::spawn(
            async move { axum::serve(listener, app).await },
        ))
    };

    tracing::info!("Logging into modem at {}", args.modem_url);
    if let Err(e) = client.login().await {
        // Keep the exporter alive so /health, /ready, and /metrics can report
        // the failure. The scrape loop will retry the login.
        tracing::error!("Initial modem login failed: {}", e);
    }

    let mut automation_outputs = automation::AutomationOutputs::new(
        args.state_file.clone(),
        args.capacity_file.clone(),
        args.state_down_threshold,
        args.state_up_threshold,
    );

    let mut spectrum_cache = SpectrumCache::default();
    let spectrum_interval = Duration::from_secs(args.spectrum_interval);
    let otlp_for_initial = otlp_client.read().await.clone();
    let initial_scrape = do_scrape(
        &client,
        otlp_for_initial.as_deref(),
        args.capacity_margin_percent,
        args.enable_spectrum,
        spectrum_interval,
        args.debug_html_dir.as_deref(),
        &mut spectrum_cache,
    )
    .await;
    if let Some(outputs) = automation_outputs.as_mut() {
        if let Err(e) = outputs.update(&initial_scrape.observation) {
            tracing::warn!("Failed to write automation output files: {}", e);
        }
    }
    record_health(&state.health, &initial_scrape).await;
    *state.metrics_text.write().await = initial_scrape.metrics_text;

    let scrape_handle = {
        let state = state.clone();
        let otlp_client = otlp_client.clone();
        let client = client.clone();
        let interval_secs = args.interval;
        let capacity_margin_percent = args.capacity_margin_percent;
        let enable_spectrum = args.enable_spectrum;
        let spectrum_interval = Duration::from_secs(args.spectrum_interval);
        let debug_html_dir = args.debug_html_dir.clone();
        tokio::spawn(async move {
            let period = Duration::from_secs(interval_secs);
            let mut interval =
                tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut automation_outputs = automation_outputs;
            let mut spectrum_cache = spectrum_cache;
            loop {
                interval.tick().await;
                let otlp = otlp_client.read().await.clone();
                let scrape = do_scrape(
                    &client,
                    otlp.as_deref(),
                    capacity_margin_percent,
                    enable_spectrum,
                    spectrum_interval,
                    debug_html_dir.as_deref(),
                    &mut spectrum_cache,
                )
                .await;
                if let Some(outputs) = automation_outputs.as_mut() {
                    if let Err(e) = outputs.update(&scrape.observation) {
                        tracing::warn!("Failed to write automation output files: {}", e);
                    }
                }
                record_health(&state.health, &scrape).await;
                let mut guard = state.metrics_text.write().await;
                *guard = scrape.metrics_text;
            }
        })
    };

    if let Some(server_handle) = server_handle {
        tokio::select! {
            r = server_handle => r??,
            r = scrape_handle => r?,
        }
    } else {
        scrape_handle.await?;
    }

    Ok(())
}

async fn do_scrape(
    client: &client::ModemClient,
    otlp: Option<&otlp::OtlpClient>,
    capacity_margin_percent: u8,
    enable_spectrum: bool,
    spectrum_interval: Duration,
    debug_html_dir: Option<&Path>,
    spectrum_cache: &mut SpectrumCache,
) -> ScrapeResult {
    let start = Instant::now();
    match client.fetch_all().await {
        Ok(pages) => {
            if enable_spectrum && spectrum_cache.should_refresh(spectrum_interval) {
                match client.fetch_spectrum().await {
                    Ok(html) => {
                        spectrum_cache.html = Some(html);
                        spectrum_cache.last_scan_at = Some(Instant::now());
                    }
                    Err(e) => {
                        tracing::warn!("Spectrum scan failed: {}", e);
                        spectrum_cache.last_scan_at = Some(Instant::now());
                    }
                }
            }

            let duration = start.elapsed().as_secs_f64();
            match parser::parse_all(
                &pages.status,
                &pages.vers,
                &pages.dhcp,
                &pages.qos,
                &pages.cm_state,
                &pages.event,
                &pages.config_params,
                &pages.product,
                &pages.dhcp_log,
                &pages.dsx,
                spectrum_cache.html.as_deref(),
                duration,
            ) {
                Ok(mut data) => {
                    data.spectrum_age_seconds = spectrum_cache
                        .last_scan_at
                        .map(|last| last.elapsed().as_secs_f64());
                    tracing::info!(
                        "Scrape OK: {} DS QAM, {} DS OFDM, {} US QAM, {} US OFDM channels ({:.1}s)",
                        data.downstream_qam.len(),
                        data.downstream_ofdm.len(),
                        data.upstream_qam.len(),
                        data.upstream_ofdm.len(),
                        duration,
                    );

                    if let Some(otlp) = otlp {
                        match otlp.push(&data, capacity_margin_percent).await {
                            Ok(()) => tracing::debug!("OTLP push OK"),
                            Err(e) => tracing::warn!("OTLP push failed: {}", e),
                        }
                    }

                    ScrapeResult {
                        metrics_text: metrics::render_metrics(&data, capacity_margin_percent),
                        observation: automation::ScrapeObservation::from_data(
                            &data,
                            capacity_margin_percent,
                        ),
                        success: true,
                        error: None,
                    }
                }
                Err(e) => {
                    let duration = start.elapsed().as_secs_f64();
                    let message = e.to_string();
                    tracing::error!("Parse error: {}", message);
                    if let Some(dir) = debug_html_dir {
                        match debug_dump::write_failed_scrape(dir, &pages, &message) {
                            Ok(path) => tracing::warn!(
                                "Wrote redacted failed-scrape HTML to {}",
                                path.display()
                            ),
                            Err(dump_err) => {
                                tracing::warn!("Failed to write debug HTML: {}", dump_err)
                            }
                        }
                    }
                    if let Some(otlp) = otlp {
                        if let Err(push_err) = otlp.push_failure(&message, duration).await {
                            tracing::warn!("OTLP failure push failed: {}", push_err);
                        }
                    }
                    ScrapeResult {
                        metrics_text: metrics::render_parse_error_metrics(
                            &message, duration, &e.status,
                        ),
                        observation: automation::ScrapeObservation::from_error(
                            message.clone(),
                            capacity_margin_percent,
                        ),
                        success: false,
                        error: Some(message),
                    }
                }
            }
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            let message = e.to_string();
            tracing::error!("Fetch error: {}", message);
            if let Some(otlp) = otlp {
                if let Err(push_err) = otlp.push_failure(&message, duration).await {
                    tracing::warn!("OTLP failure push failed: {}", push_err);
                }
            }
            ScrapeResult {
                metrics_text: metrics::render_error_metrics(&message, duration),
                observation: automation::ScrapeObservation::from_error(
                    message.clone(),
                    capacity_margin_percent,
                ),
                success: false,
                error: Some(message),
            }
        }
    }
}

fn readiness_age(health: &HealthState, stale_after: Duration) -> Option<Duration> {
    health
        .last_success
        .map(|last| last.elapsed())
        .filter(|age| *age <= stale_after)
}

async fn record_health(health: &RwLock<HealthState>, scrape: &ScrapeResult) {
    let mut guard = health.write().await;
    if scrape.success {
        guard.last_success = Some(Instant::now());
        guard.last_error = None;
    } else {
        guard.last_error = scrape.error.clone();
    }
}

fn validate_args(args: &Args) -> Result<()> {
    if args.password.is_empty() {
        bail!("--password or MODEM_PASSWORD must be set");
    }
    for header in &args.otlp_headers {
        if !header.contains('=') {
            bail!("invalid --otlp-header (expected KEY=VALUE): {header}");
        }
    }
    if args.interval == 0 {
        bail!("--interval must be greater than zero");
    }
    if args.enable_spectrum && args.spectrum_interval == 0 {
        bail!("--spectrum-interval must be greater than zero when --enable-spectrum is set");
    }
    if args.capacity_margin_percent > 100 {
        bail!("--capacity-margin-percent must be between 0 and 100");
    }
    if args.connect_timeout == 0 {
        bail!("--connect-timeout must be greater than zero");
    }
    if args.request_timeout == 0 {
        bail!("--request-timeout must be greater than zero");
    }
    if args.state_down_threshold == 0 || args.state_up_threshold == 0 {
        bail!("--state-down-threshold and --state-up-threshold must be greater than zero");
    }
    if args.readiness_stale_after == Some(0) {
        bail!("--readiness-stale-after must be greater than zero");
    }
    let fingerprint_set = args
        .tls_cert_fingerprint
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if args.ca_cert.is_some() && (args.insecure_tls || fingerprint_set) {
        bail!("--ca-cert cannot be combined with --insecure-tls or --tls-cert-fingerprint");
    }
    Ok(())
}

async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    let body = state.metrics_text.read().await.clone();
    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn ready_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    let health = state.health.read().await;
    let stale_for = health.last_success.map(|last| last.elapsed());
    let ready = readiness_age(&health, state.ready_stale_after).is_some();
    if ready {
        (
            axum::http::StatusCode::OK,
            format!(
                "ready\nlast_success_seconds_ago={:.1}\n",
                stale_for.map(|d| d.as_secs_f64()).unwrap_or_default()
            ),
        )
    } else {
        let reason = match stale_for {
            None => "no successful scrape yet".to_string(),
            Some(age) => format!(
                "last successful scrape was {:.1}s ago (threshold {}s)",
                age.as_secs_f64(),
                state.ready_stale_after.as_secs()
            ),
        };
        let error = health
            .last_error
            .as_deref()
            .map(|e| format!("\nlast_error={}", e))
            .unwrap_or_default();
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            format!("not ready\nreason={}{}\n", reason, error),
        )
    }
}

async fn landing_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(
        "<!DOCTYPE html><html><head><title>ARRIS CM3500B CE Exporter</title></head>\
         <body><h1>ARRIS CM3500B CE Prometheus Exporter</h1>\
         <p><a href=\"/metrics\">Metrics</a> | <a href=\"/health\">Health</a> | \
         <a href=\"/ready\">Readiness</a></p>\
         </body></html>",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Args {
        let mut argv = vec!["cm3500-exporter", "--password", "test"];
        argv.extend_from_slice(extra);
        Args::try_parse_from(argv).expect("arguments should parse")
    }

    #[test]
    fn rejects_empty_password_and_malformed_otlp_headers() {
        let mut empty_password = args(&[]);
        empty_password.password = String::new();
        assert!(validate_args(&empty_password).is_err());
        assert!(validate_args(&args(&["--otlp-header", "not-a-pair"])).is_err());
        assert!(validate_args(&args(&["--otlp-header", "Authorization=Basic x"])).is_ok());
    }

    #[test]
    fn rejects_zero_intervals_and_invalid_margins() {
        assert!(validate_args(&args(&["--interval", "0"])).is_err());
        assert!(validate_args(&args(&["--capacity-margin-percent", "101"])).is_err());
        assert!(validate_args(&args(&["--capacity-margin-percent", "100"])).is_ok());
        assert!(validate_args(&args(&["--connect-timeout", "0"])).is_err());
        assert!(validate_args(&args(&["--request-timeout", "0"])).is_err());
    }

    #[test]
    fn zero_spectrum_interval_only_rejected_when_enabled() {
        assert!(validate_args(&args(&["--spectrum-interval", "0"])).is_ok());
        assert!(validate_args(&args(&["--enable-spectrum", "--spectrum-interval", "0"])).is_err());
    }

    #[tokio::test]
    async fn spectrum_scan_failure_does_not_fail_a_healthy_scrape() {
        async fn handler(
            method: axum::http::Method,
            uri: axum::http::Uri,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;
            match (method, uri.path()) {
                (axum::http::Method::POST, "/cgi-bin/spectrum_cgi") => {
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "").into_response()
                }
                (axum::http::Method::GET, "/cgi-bin/status_cgi") => (
                    axum::http::StatusCode::OK,
                    r#"
                    <table>
                      <tr><td>CM Status:</td><td>OPERATIONAL</td></tr>
                      <tr><td>Downstream 1</td><td>3</td><td>570.00 MHz</td><td>2.20 dBmV</td><td>40.37 dB</td><td>256QAM</td><td>2858</td><td>12</td><td>0</td></tr>
                    </table>
                    "#,
                )
                    .into_response(),
                (axum::http::Method::GET, "/cgi-bin/vers_cgi") => {
                    (axum::http::StatusCode::OK, "MODEL: CM3500B").into_response()
                }
                _ => (axum::http::StatusCode::OK, "").into_response(),
            }
        }

        let app = axum::Router::new().fallback(handler);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = client::ModemClient::new(
            &format!("http://{}", addr),
            "admin",
            "pw",
            &client::ClientOptions {
                connect_timeout: Duration::from_secs(1),
                request_timeout: Duration::from_secs(2),
                insecure_tls: false,
                ca_cert: None,
                cert_fingerprint: None,
            },
        )
        .unwrap();
        let mut cache = SpectrumCache::default();
        let result = do_scrape(
            &client,
            None,
            95,
            true,
            Duration::from_secs(0),
            None,
            &mut cache,
        )
        .await;

        assert!(result.success);
        assert!(result.metrics_text.contains("cm3500_up 1"));
        assert!(result
            .metrics_text
            .contains("cm3500_downstream_qam_power_dbmv"));
    }

    #[test]
    fn readiness_requires_a_recent_success() {
        let stale_after = Duration::from_secs(60);
        let never = HealthState::default();
        assert_eq!(readiness_age(&never, stale_after), None);

        let recent = HealthState {
            last_success: Some(Instant::now() - Duration::from_secs(5)),
            last_error: None,
        };
        assert!(readiness_age(&recent, stale_after).is_some());

        let stale = HealthState {
            last_success: Some(Instant::now() - Duration::from_secs(120)),
            last_error: Some("timeout".into()),
        };
        assert_eq!(readiness_age(&stale, stale_after), None);
    }

    #[test]
    fn tls_options_are_compatible_with_compose_fallback() {
        assert!(validate_args(&args(&["--insecure-tls", "--ca-cert", "/tmp/ca.pem"])).is_err());
        assert!(validate_args(&args(&[
            "--ca-cert",
            "/tmp/ca.pem",
            "--tls-cert-fingerprint",
            "aa"
        ]))
        .is_err());
        // Compose always passes both; a non-empty fingerprint takes precedence.
        assert!(validate_args(&args(&["--insecure-tls", "--tls-cert-fingerprint", "aa"])).is_ok());
        assert!(validate_args(&args(&["--insecure-tls", "--tls-cert-fingerprint", ""])).is_ok());
        assert!(validate_args(&args(&["--tls-cert-fingerprint", "aa"])).is_ok());
    }
}
