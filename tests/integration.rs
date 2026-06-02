use cm3500_b_ce_exporter::metrics;
use cm3500_b_ce_exporter::parser;

/// Full integration test: parse real-ish HTML pages, render metrics, verify output
#[test]
fn test_full_pipeline_status_to_metrics() {
    let status_html = include_str!("fixtures/status_cgi.html");
    let vers_html = include_str!("fixtures/vers_cgi.html");
    let dhcp_html = include_str!("fixtures/dhcp_cgi.html");
    let qos_html = include_str!("fixtures/qos_cgi.html");
    let cm_state_html = include_str!("fixtures/cm_state_cgi.html");
    let event_html = include_str!("fixtures/event_cgi.html");
    let config_params_html = include_str!("fixtures/config_params_cgi.html");
    let product_html = r#"<tr><td width="160">Ethernet Phy Type</td><td>1x2.5G-GPY21X switch</td></tr>
        <br><b>[3] COMMON_COMPONENTS</b>: Enabled <br>
        <table><tr><td>[ 0] CLI: Enabled </td><td>[ 1] ENVOY: Enabled </td></tr></table>"#;
    let spectrum_html = r#"<script>var spectrum_data = ["1045a640007270e0000000040001c9c300000001fa24fa10fb64fbdc"];</script>"#;

    let data = parser::parse_all(
        status_html,
        vers_html,
        dhcp_html,
        qos_html,
        cm_state_html,
        event_html,
        config_params_html,
        product_html,
        Some(spectrum_html),
        1.5,
    )
    .expect("parse should succeed");

    assert_eq!(data.downstream_qam.len(), 32);
    assert_eq!(data.downstream_ofdm.len(), 1);
    assert_eq!(data.upstream_qam.len(), 4);
    assert_eq!(data.upstream_ofdm.len(), 1);
    assert_eq!(data.cm_status, "OPERATIONAL");
    assert!(data.uptime_seconds.is_some());
    assert_eq!(data.cpe_dynamic, 1);
    assert_eq!(data.cpe_static, 0);
    assert_eq!(data.dhcp_state, "bound");
    assert!(!data.events.is_empty());
    assert_eq!(data.interfaces.len(), 1);
    assert_eq!(data.interfaces[0].name, "CABLE");
    assert_eq!(data.interfaces[0].state, "Up");
    assert_eq!(data.product_info.ethernet_phy_type, "1x2.5G-GPY21X switch");
    assert_eq!(data.spectrum.len(), 1);

    let output = metrics::render_metrics(&data);

    // Verify key metrics appear in output
    assert!(output.contains("cm3500_up 1"));
    assert!(output.contains("cm3500_scrape_duration_seconds 1.5"));
    assert!(output.contains("cm3500_cm_status{status=\"OPERATIONAL\"}"));
    assert!(output.contains("cm3500_downstream_qam_power_dbmv{"));
    assert!(output.contains("cm3500_downstream_qam_snr_db{"));
    assert!(output.contains("cm3500_downstream_qam_octets_total{"));
    assert!(output.contains("cm3500_downstream_qam_corrected_total{"));
    assert!(output.contains("cm3500_downstream_qam_uncorrectable_total{"));
    assert!(output.contains("cm3500_downstream_ofdm_rxmer_db{"));
    assert!(output.contains("cm3500_upstream_qam_power_dbmv{"));
    assert!(output.contains("cm3500_upstream_ofdma_tx_power_dbmv{"));
    assert!(output.contains("cm3500_upstream_ofdma_active_subcarrier_range_mhz{"));
    assert!(output.contains("cm3500_dhcp_lease_remaining_seconds"));
    assert!(output.contains("cm3500_dhcp_state{state=\"bound\"}"));
    assert!(output.contains("cm3500_cpe_dynamic 1"));
    assert!(output.contains("cm3500_cpe_static 0"));
    assert!(output.contains("cm3500_interface_up{"));
    assert!(output.contains("cm3500_event_log_entries{"));
    assert!(output.contains("cm3500_event_t3_timeouts"));
    assert!(output.contains("cm3500_event_upstream_active_profile{"));
    assert!(output.contains("cm3500_event_ofdma_profile_id{"));
    assert!(output.contains("cm3500_docsis_state{"));
    assert!(output.contains("cm3500_bpi_state{"));
    assert!(output.contains("cm3500_tod_state{"));

    // Verify service flow config metrics
    assert!(output.contains("cm3500_qos_max_traffic_rate_kbps{"));
    assert!(output.contains("cm3500_product_ethernet_phy_info{"));
    assert!(output.contains("cm3500_product_logging_components_enabled{"));
    assert!(output.contains("cm3500_spectrum_chunks 1"));
    assert!(output.contains("cm3500_spectrum_bin_power_dbmv{"));
    assert!(output.contains("cm3500_qos_min_reserved_rate_kbps{"));
    assert!(output.contains("cm3500_qos_max_traffic_burst{"));
    assert!(output.contains("cm3500_qos_traffic_priority{"));
    assert!(output.contains("direction=\"Upstream\""));
    assert!(output.contains("direction=\"Downstream\""));

    // Verify TYPE headers present
    assert!(output.contains("# TYPE cm3500_up gauge"));
    assert!(output.contains("# TYPE cm3500_downstream_qam_octets_total counter"));
    assert!(output.contains("# TYPE cm3500_qos_packets_total counter"));

    // Verify info labels
    assert!(output.contains("cm3500_info{"));
    assert!(output.contains("model=\"CM3500B\""));
}

#[test]
fn test_error_metrics() {
    let output = metrics::render_error_metrics("connection refused", 2.5);
    assert!(output.contains("cm3500_up 0"));
    assert!(output.contains("cm3500_scrape_duration_seconds 2.5"));
    assert!(output.contains("cm3500_scrape_error{error=\"connection refused\"}"));
}

#[test]
fn test_metrics_format_valid() {
    let status_html = include_str!("fixtures/status_cgi.html");
    let vers_html = include_str!("fixtures/vers_cgi.html");
    let dhcp_html = include_str!("fixtures/dhcp_cgi.html");
    let qos_html = include_str!("fixtures/qos_cgi.html");
    let cm_state_html = include_str!("fixtures/cm_state_cgi.html");
    let event_html = include_str!("fixtures/event_cgi.html");
    let config_params_html = include_str!("fixtures/config_params_cgi.html");
    let product_html = "";

    let data = parser::parse_all(
        status_html,
        vers_html,
        dhcp_html,
        qos_html,
        cm_state_html,
        event_html,
        config_params_html,
        product_html,
        None,
        1.0,
    )
    .unwrap();

    let output = metrics::render_metrics(&data);

    // Every non-comment, non-empty line should be parseable as NAME{labels} VALUE
    for line in output.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Must contain at least "metric_name value" or "metric_name{...} value"
        let parts: Vec<&str> = line.rsplitn(2, ' ').collect();
        assert_eq!(
            parts.len(),
            2,
            "Line should split into metric+value: {}",
            line
        );
        let value = parts[0];
        let value_f: Result<f64, _> = value.parse();
        assert!(
            value_f.is_ok(),
            "Value should be a number: '{}' in line '{}'",
            value,
            line
        );
    }
}
