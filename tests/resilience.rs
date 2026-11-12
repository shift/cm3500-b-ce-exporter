use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::IntoResponse;
use cm3500_b_ce_exporter::client::{ClientOptions, ModemClient};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
struct MockBrowser {
    status_hits: Arc<AtomicUsize>,
    logins: Arc<AtomicUsize>,
    expire_first_status: bool,
    delay: Option<Duration>,
}

async fn start_mock(mock: MockBrowser) -> String {
    let app = axum::Router::new().fallback(handle).with_state(mock);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{}", addr)
}

async fn handle(State(mock): State<MockBrowser>, method: Method, uri: Uri) -> impl IntoResponse {
    if let Some(delay) = mock.delay {
        tokio::time::sleep(delay).await;
    }

    let path = uri.path();
    match (method, path) {
        (Method::POST, "/cgi-bin/login_cgi") => {
            mock.logins.fetch_add(1, Ordering::SeqCst);
            (StatusCode::OK, "csrf_token=test".to_string())
        }
        (Method::GET, "/cgi-bin/status_cgi") => {
            let hit = mock.status_hits.fetch_add(1, Ordering::SeqCst);
            if mock.expire_first_status && hit == 0 {
                (
                    StatusCode::OK,
                    "<meta http-equiv=\"refresh\" content=\"0;url=login_cgi\">".to_string(),
                )
            } else {
                (StatusCode::OK, "OPERATIONAL".to_string())
            }
        }
        _ => (StatusCode::OK, "ok".to_string()),
    }
}

fn options(request_timeout: Duration) -> ClientOptions {
    ClientOptions {
        connect_timeout: Duration::from_secs(1),
        request_timeout,
        insecure_tls: false,
        ca_cert: None,
        cert_fingerprint: None,
    }
}

#[tokio::test]
async fn reauthenticates_once_when_session_expires() {
    let mock = MockBrowser {
        expire_first_status: true,
        ..Default::default()
    };
    let status_hits = mock.status_hits.clone();
    let logins = mock.logins.clone();
    let base = start_mock(mock).await;

    let client = ModemClient::new(&base, "admin", "pw", &options(Duration::from_secs(2)))
        .expect("client should build");
    let pages = client.fetch_all().await.expect("fetch should recover");

    assert_eq!(pages.status, "OPERATIONAL");
    assert_eq!(status_hits.load(Ordering::SeqCst), 2);
    assert_eq!(logins.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn request_timeout_bounds_a_hung_modem() {
    let mock = MockBrowser {
        delay: Some(Duration::from_secs(5)),
        ..Default::default()
    };
    let base = start_mock(mock).await;
    let client = ModemClient::new(&base, "admin", "pw", &options(Duration::from_millis(150)))
        .expect("client should build");

    let started = Instant::now();
    let err = client
        .fetch_page("status_cgi")
        .await
        .expect_err("request should time out");
    let elapsed = started.elapsed();

    assert!(elapsed < Duration::from_secs(2), "elapsed: {:?}", elapsed);
    let chain = format!("{err:#}").to_lowercase();
    assert!(
        chain.contains("timed out") || chain.contains("timeout"),
        "unexpected error: {chain}"
    );
}
