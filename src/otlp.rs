use crate::parser::ScrapedData;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct OtlpClient {
    client: Client,
    endpoint: String,
    headers: Vec<(String, String)>,
    resource_attrs: Vec<Value>,
}

impl OtlpClient {
    pub fn new(endpoint: &str, headers: Vec<(String, String)>, data: &ScrapedData) -> Self {
        let resource_attrs = vec![
            str_attr("service.name", "cm3500-exporter"),
            str_attr("service.version", env!("CARGO_PKG_VERSION")),
            str_attr("service.instance.id", &data.version_info.serial),
            str_attr("cm3500.model", &data.version_info.model),
            str_attr("cm3500.firmware", &data.version_info.software_rev),
            str_attr("cm3500.serial", &data.version_info.serial),
        ];

        Self {
            client: Client::new(),
            endpoint: endpoint.to_string(),
            headers,
            resource_attrs,
        }
    }

    pub async fn push(&self, data: &ScrapedData) -> Result<()> {
        let now = now_nanos();
        let mut metrics: Vec<Value> = Vec::new();

        // Scrape health
        metrics.push(gauge(
            "cm3500_up",
            "Whether the modem scrape was successful",
            "",
            vec![f64_dp(1.0, vec![], &now)],
        ));
        metrics.push(gauge(
            "cm3500_scrape_duration_seconds",
            "Duration of the last scrape in seconds",
            "s",
            vec![f64_dp(data.scrape_duration_secs, vec![], &now)],
        ));

        // Uptime
        if let Some(uptime) = data.uptime_seconds {
            metrics.push(gauge(
                "cm3500_uptime_seconds",
                "System uptime in seconds",
                "s",
                vec![f64_dp(uptime as f64, vec![], &now)],
            ));
        }

        // CM Status
        if !data.cm_status.is_empty() {
            metrics.push(gauge(
                "cm3500_cm_status",
                "Cable modem operational status",
                "",
                vec![f64_dp(1.0, vec![str_attr("status", &data.cm_status)], &now)],
            ));
        }

        // Interfaces
        for iface in &data.interfaces {
            let up = if iface.state == "Up" { 1.0 } else { 0.0 };
            metrics.push(gauge(
                "cm3500_interface_up",
                "Interface operational status (1 = Up)",
                "",
                vec![f64_dp(
                    up,
                    vec![
                        str_attr("name", &iface.name),
                        str_attr("provisioned", &iface.provisioned),
                        str_attr("mac_address", &iface.mac_address),
                    ],
                    &now,
                )],
            ));
        }

        // CPE counts
        metrics.push(gauge(
            "cm3500_cpe_static",
            "Number of static CPE devices detected",
            "",
            vec![f64_dp(data.cpe_static as f64, vec![], &now)],
        ));
        metrics.push(gauge(
            "cm3500_cpe_dynamic",
            "Number of dynamic CPE devices detected",
            "",
            vec![f64_dp(data.cpe_dynamic as f64, vec![], &now)],
        ));

        // DHCP
        if !data.dhcp_state.is_empty() {
            metrics.push(gauge(
                "cm3500_dhcp_state",
                "DHCP state of the cable modem",
                "",
                vec![f64_dp(1.0, vec![str_attr("state", &data.dhcp_state)], &now)],
            ));
        }
        for (name, desc, opt_val) in [
            (
                "cm3500_dhcp_lease_total_seconds",
                "Total DHCP lease duration in seconds",
                data.dhcp_info.lease_total_secs,
            ),
            (
                "cm3500_dhcp_lease_remaining_seconds",
                "DHCP lease time remaining in seconds",
                data.dhcp_info.lease_remaining_secs,
            ),
            (
                "cm3500_dhcp_rebind_remaining_seconds",
                "DHCP rebind time remaining in seconds",
                data.dhcp_info.rebind_remaining_secs,
            ),
            (
                "cm3500_dhcp_renew_remaining_seconds",
                "DHCP renew time remaining in seconds",
                data.dhcp_info.renew_remaining_secs,
            ),
        ] {
            if let Some(v) = opt_val {
                metrics.push(gauge(name, desc, "s", vec![f64_dp(v as f64, vec![], &now)]));
            }
        }

        // Downstream QAM
        let mut ds_power = Vec::new();
        let mut ds_snr = Vec::new();
        let mut ds_octets = Vec::new();
        let mut ds_corrected = Vec::new();
        let mut ds_uncorrectable = Vec::new();

        for ch in &data.downstream_qam {
            let attrs = vec![
                int_attr("channel", ch.channel),
                int_attr("dcid", ch.dcid),
                str_attr("frequency_mhz", &format!("{:.2}", ch.freq_mhz)),
                str_attr("modulation", &ch.modulation),
            ];
            ds_power.push(f64_dp(ch.power_dbmv, attrs.clone(), &now));
            ds_snr.push(f64_dp(ch.snr_db, attrs.clone(), &now));
            ds_octets.push(i64_dp(ch.octets as i64, attrs.clone(), &now));
            ds_corrected.push(i64_dp(ch.correcteds as i64, attrs.clone(), &now));
            ds_uncorrectable.push(i64_dp(ch.uncorrectables as i64, attrs, &now));
        }

        if !ds_power.is_empty() {
            metrics.push(gauge(
                "cm3500_downstream_qam_power_dbmv",
                "Downstream SC-QAM power level in dBmV",
                "dBmV",
                ds_power,
            ));
            metrics.push(gauge(
                "cm3500_downstream_qam_snr_db",
                "Downstream SC-QAM signal-to-noise ratio in dB",
                "dB",
                ds_snr,
            ));
            metrics.push(sum(
                "cm3500_downstream_qam_octets_total",
                "Total octets received on downstream SC-QAM channel",
                "",
                ds_octets,
                &now,
            ));
            metrics.push(sum(
                "cm3500_downstream_qam_corrected_total",
                "Total corrected errors on downstream SC-QAM channel",
                "",
                ds_corrected,
                &now,
            ));
            metrics.push(sum(
                "cm3500_downstream_qam_uncorrectable_total",
                "Total uncorrectable errors on downstream SC-QAM channel",
                "",
                ds_uncorrectable,
                &now,
            ));
        }

        // Downstream OFDM
        for ch in &data.downstream_ofdm {
            let ch_attrs = vec![
                int_attr("channel", ch.channel),
                str_attr("fft_type", &ch.fft_type),
            ];
            metrics.push(gauge(
                "cm3500_downstream_ofdm_channel_width_mhz",
                "Downstream OFDM channel width in MHz",
                "MHz",
                vec![f64_dp(ch.channel_width_mhz, ch_attrs.clone(), &now)],
            ));
            metrics.push(gauge(
                "cm3500_downstream_ofdm_active_subcarriers",
                "Number of active subcarriers on downstream OFDM channel",
                "",
                vec![f64_dp(
                    ch.active_subcarriers as f64,
                    vec![int_attr("channel", ch.channel)],
                    &now,
                )],
            ));
            for (typ, val) in [
                ("pilot", ch.rxmer_pilot_db),
                ("plc", ch.rxmer_plc_db),
                ("data", ch.rxmer_data_db),
            ] {
                metrics.push(gauge(
                    "cm3500_downstream_ofdm_rxmer_db",
                    "Downstream OFDM receive modulation error ratio in dB",
                    "dB",
                    vec![f64_dp(
                        val,
                        vec![int_attr("channel", ch.channel), str_attr("type", typ)],
                        &now,
                    )],
                ));
            }
        }

        // Upstream QAM
        let mut us_power = Vec::new();
        let mut us_sym_rate = Vec::new();

        for ch in &data.upstream_qam {
            let attrs = vec![
                int_attr("channel", ch.channel),
                int_attr("ucid", ch.ucid),
                str_attr("frequency_mhz", &format!("{:.2}", ch.freq_mhz)),
                str_attr("modulation", &ch.modulation),
                str_attr("channel_type", &ch.channel_type),
            ];
            us_power.push(f64_dp(ch.power_dbmv, attrs.clone(), &now));
            us_sym_rate.push(f64_dp(
                ch.symbol_rate_ksym,
                vec![int_attr("channel", ch.channel), int_attr("ucid", ch.ucid)],
                &now,
            ));
        }

        if !us_power.is_empty() {
            metrics.push(gauge(
                "cm3500_upstream_qam_power_dbmv",
                "Upstream SC-QAM transmit power in dBmV",
                "dBmV",
                us_power,
            ));
            metrics.push(gauge(
                "cm3500_upstream_qam_symbol_rate_ksym",
                "Upstream SC-QAM symbol rate in kSym/s",
                "kSym/s",
                us_sym_rate,
            ));
        }

        // Upstream OFDMA
        for ch in &data.upstream_ofdm {
            let ch_attrs = vec![
                int_attr("channel", ch.channel),
                str_attr("fft_type", &ch.fft_type),
            ];
            metrics.push(gauge(
                "cm3500_upstream_ofdma_tx_power_dbmv",
                "Upstream OFDMA transmit power in dBmV",
                "dBmV",
                vec![f64_dp(ch.tx_power_dbmv, ch_attrs, &now)],
            ));
            metrics.push(gauge(
                "cm3500_upstream_ofdma_channel_width_mhz",
                "Upstream OFDMA channel width in MHz",
                "MHz",
                vec![f64_dp(
                    ch.channel_width_mhz,
                    vec![int_attr("channel", ch.channel)],
                    &now,
                )],
            ));
            metrics.push(gauge(
                "cm3500_upstream_ofdma_active_subcarriers",
                "Number of active subcarriers on upstream OFDMA channel",
                "",
                vec![f64_dp(
                    ch.active_subcarriers as f64,
                    vec![int_attr("channel", ch.channel)],
                    &now,
                )],
            ));
        }

        // QoS
        if !data.qos_flows.is_empty() {
            let qos_dps: Vec<Value> = data
                .qos_flows
                .iter()
                .map(|f| {
                    i64_dp(
                        f.packets as i64,
                        vec![
                            int_attr_i64("sfid", f.sfid as i64),
                            str_attr("direction", &f.direction),
                            str_attr("primary", if f.primary { "true" } else { "false" }),
                            str_attr("service_class", &f.service_class),
                        ],
                        &now,
                    )
                })
                .collect();
            metrics.push(sum(
                "cm3500_qos_packets_total",
                "Total packets on QoS service flow",
                "",
                qos_dps,
                &now,
            ));
        }

        // DOCSIS state
        if !data.cm_state.overall_state.is_empty() {
            metrics.push(gauge(
                "cm3500_docsis_state",
                "Overall DOCSIS registration state",
                "",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("state", &data.cm_state.overall_state)],
                    &now,
                )],
            ));
        }
        if !data.cm_state.phases.is_empty() {
            let phase_dps: Vec<Value> = data
                .cm_state
                .phases
                .iter()
                .map(|p| {
                    let phase_normalized = p.phase.to_lowercase().replace(['-', ' ', '/'], "_");
                    f64_dp(
                        1.0,
                        vec![
                            str_attr("phase", &phase_normalized),
                            str_attr("status", &p.status),
                        ],
                        &now,
                    )
                })
                .collect();
            metrics.push(gauge(
                "cm3500_docsis_phase",
                "DOCSIS registration phase status",
                "",
                phase_dps,
            ));
        }
        if !data.cm_state.bpi_status.is_empty() {
            metrics.push(gauge(
                "cm3500_bpi_state",
                "Baseline Privacy Interface state",
                "",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("status", &data.cm_state.bpi_status)],
                    &now,
                )],
            ));
        }
        if !data.cm_state.tod_status.is_empty() {
            metrics.push(gauge(
                "cm3500_tod_state",
                "Time of Day acquisition state",
                "",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("status", &data.cm_state.tod_status)],
                    &now,
                )],
            ));
        }

        // Event log
        if !data.events.is_empty() {
            let mut counts: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
            for e in &data.events {
                *counts.entry(&e.event_id).or_insert(0) += 1;
            }
            let event_dps: Vec<Value> = counts
                .iter()
                .map(|(id, count)| f64_dp(*count as f64, vec![str_attr("event_id", id)], &now))
                .collect();
            metrics.push(gauge(
                "cm3500_event_log_entries",
                "Number of entries in the modem event log by event ID",
                "",
                event_dps,
            ));

            // Dedicated critical event metrics
            let mut t3 = 0u32;
            let mut t4 = 0u32;
            let mut dhcp_fail = 0u32;
            let mut dhcp_warn = 0u32;
            let mut profile_chg = 0u32;
            for e in &data.events {
                match e.event_id.as_str() {
                    "82000200" => t3 += 1,
                    "82000300" => t4 += 1,
                    "68001202" => dhcp_fail += 1,
                    "68010300" => dhcp_warn += 1,
                    "67061601" => profile_chg += 1,
                    _ => {}
                }
            }
            for (name, desc, val) in [
                (
                    "cm3500_event_t3_timeout_total",
                    "T3 timeouts in event log",
                    t3,
                ),
                (
                    "cm3500_event_t4_timeout_total",
                    "T4 timeouts in event log",
                    t4,
                ),
                (
                    "cm3500_event_dhcp_failure_total",
                    "DHCP failures in event log",
                    dhcp_fail,
                ),
                (
                    "cm3500_event_dhcp_renew_warning_total",
                    "DHCP renew warnings in event log",
                    dhcp_warn,
                ),
                (
                    "cm3500_event_profile_change_total",
                    "US profile assignment changes in event log",
                    profile_chg,
                ),
            ] {
                metrics.push(gauge(
                    name,
                    desc,
                    "",
                    vec![f64_dp(val as f64, vec![], &now)],
                ));
            }
        }

        // Build the full OTLP request body
        let body = json!({
            "resourceMetrics": [{
                "resource": { "attributes": self.resource_attrs },
                "scopeMetrics": [{
                    "scope": {
                        "name": "cm3500-exporter",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "metrics": metrics
                }]
            }]
        });

        let mut req = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&body);

        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OTLP push failed: HTTP {} - {}", status, body));
        }

        Ok(())
    }
}

// Helpers

fn now_nanos() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

fn str_attr(key: &str, value: &str) -> Value {
    json!({"key": key, "value": {"stringValue": value}})
}

fn int_attr(key: &str, value: u32) -> Value {
    json!({"key": key, "value": {"intValue": value.to_string()}})
}

fn int_attr_i64(key: &str, value: i64) -> Value {
    json!({"key": key, "value": {"intValue": value.to_string()}})
}

fn f64_dp(value: f64, attributes: Vec<Value>, time_nanos: &str) -> Value {
    json!({
        "timeUnixNano": time_nanos,
        "asDouble": value,
        "attributes": attributes
    })
}

fn i64_dp(value: i64, attributes: Vec<Value>, time_nanos: &str) -> Value {
    json!({
        "timeUnixNano": time_nanos,
        "asInt": value.to_string(),
        "attributes": attributes
    })
}

fn gauge(name: &str, description: &str, unit: &str, data_points: Vec<Value>) -> Value {
    json!({
        "name": name,
        "description": description,
        "unit": unit,
        "gauge": { "dataPoints": data_points }
    })
}

fn sum(
    name: &str,
    description: &str,
    unit: &str,
    data_points: Vec<Value>,
    start_nanos: &str,
) -> Value {
    let dps: Vec<Value> = data_points
        .into_iter()
        .map(|dp| {
            let mut obj = dp.as_object().unwrap().clone();
            obj.insert(
                "startTimeUnixNano".to_string(),
                Value::String(start_nanos.to_string()),
            );
            Value::Object(obj)
        })
        .collect();
    json!({
        "name": name,
        "description": description,
        "unit": unit,
        "sum": {
            "dataPoints": dps,
            "isMonotonic": true,
            "aggregationTemporality": 2
        }
    })
}
