use cm3500_b_ce_exporter::{client, metrics, otlp, parser};

use anyhow::{bail, Result};
use clap::Parser;
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

    /// Modem login password
    #[arg(long, default_value_t = String::new())]
    password: String,

    /// Listen address for the Prometheus metrics server
    #[arg(long, default_value = "0.0.0.0:10044")]
    listen: String,

    /// Disable the Prometheus /metrics HTTP endpoint entirely
    #[arg(long)]
    disable_prometheus: bool,

    /// Scrape interval in seconds
    #[arg(long, default_value_t = 30)]
    interval: u64,

    /// OTLP HTTP base URL or /v1/metrics endpoint to push metrics and logs to
    #[arg(long)]
    otlp_endpoint: Option<String>,

    /// OTLP HTTP header in KEY=VALUE format (can be repeated)
    #[arg(long = "otlp-header", value_name = "KEY=VALUE")]
    otlp_headers: Vec<String>,
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

    if args.disable_prometheus && args.otlp_endpoint.is_none() {
        bail!("--disable-prometheus requires --otlp-endpoint");
    }

    let client = client::ModemClient::new(&args.modem_url, &args.username, &args.password)?;

    tracing::info!("Logging into modem at {}", args.modem_url);
    client.login().await?;

    // Initial scrape
    let initial_metrics = do_scrape(&client, None).await;
    let metrics_text: Arc<RwLock<String>> = Arc::new(RwLock::new(initial_metrics));

    // Parse OTLP headers
    let otlp_headers: Vec<(String, String)> = args
        .otlp_headers
        .iter()
        .filter_map(|h| {
            let (k, v) = h.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect();

    let otlp_client: Arc<RwLock<Option<otlp::OtlpClient>>> =
        if let Some(endpoint) = &args.otlp_endpoint {
            tracing::info!("OTLP push enabled, endpoint: {}", endpoint);
            Arc::new(RwLock::new(Some(otlp::OtlpClient::new_fallback(
                endpoint,
                otlp_headers,
                &args.modem_url,
            ))))
        } else {
            Arc::new(RwLock::new(None))
        };

    // Background scraper
    let scrape_handle = {
        let metrics_text = metrics_text.clone();
        let otlp_client = otlp_client.clone();
        let client = client.clone();
        let interval_secs = args.interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let otlp = otlp_client.read().await;
                let otlp_ref = otlp.as_ref();
                let new_text = do_scrape(&client, otlp_ref).await;
                let mut guard = metrics_text.write().await;
                *guard = new_text;
            }
        })
    };

    if args.disable_prometheus {
        tracing::info!("Prometheus HTTP endpoint disabled");
        scrape_handle.await?;
    } else {
        let app = axum::Router::new()
            .route("/metrics", axum::routing::get(metrics_handler))
            .route("/health", axum::routing::get(health_handler))
            .route("/", axum::routing::get(landing_handler))
            .with_state(metrics_text);

        let listener = tokio::net::TcpListener::bind(&args.listen).await?;
        tracing::info!("Listening on {}", args.listen);

        let server_handle = axum::serve(listener, app);

        tokio::select! {
            r = server_handle => r?,
            r = scrape_handle => r?,
        }
    }

    Ok(())
}

async fn do_scrape(client: &client::ModemClient, otlp: Option<&otlp::OtlpClient>) -> String {
    let start = Instant::now();
    match client.fetch_all().await {
        Ok(pages) => {
            let duration = start.elapsed().as_secs_f64();
            match parser::parse_all(
                &pages.status,
                &pages.vers,
                &pages.dhcp,
                &pages.qos,
                &pages.cm_state,
                &pages.event,
                &pages.config_params,
                duration,
            ) {
                Ok(data) => {
                    tracing::info!(
                        "Scrape OK: {} DS QAM, {} DS OFDM, {} US QAM, {} US OFDM channels ({:.1}s)",
                        data.downstream_qam.len(),
                        data.downstream_ofdm.len(),
                        data.upstream_qam.len(),
                        data.upstream_ofdm.len(),
                        duration,
                    );

                    // Push to OTLP if configured
                    if let Some(otlp) = otlp {
                        match otlp.push(&data).await {
                            Ok(()) => tracing::debug!("OTLP push OK"),
                            Err(e) => tracing::warn!("OTLP push failed: {}", e),
                        }
                    }

                    metrics::render_metrics(&data)
                }
                Err(e) => {
                    let duration = start.elapsed().as_secs_f64();
                    tracing::error!("Parse error: {}", e);
                    metrics::render_error_metrics(&e.to_string(), duration)
                }
            }
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            tracing::error!("Fetch error: {}", e);
            metrics::render_error_metrics(&e.to_string(), duration)
        }
    }
}

async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<String>>>,
) -> impl axum::response::IntoResponse {
    let body = state.read().await.clone();
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

async fn landing_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(
        "<!DOCTYPE html><html><head><title>ARRIS CM3500B CE Exporter</title></head>\
         <body><h1>ARRIS CM3500B CE Prometheus Exporter</h1>\
         <p><a href=\"/metrics\">Metrics</a> | <a href=\"/health\">Health</a></p>\
         </body></html>",
    )
}
