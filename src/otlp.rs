use crate::parser::{EventLogEntry, ScrapedData};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct OtlpClient {
    client: Client,
    metrics_endpoint: String,
    logs_endpoint: String,
    headers: Vec<(String, String)>,
    resource_attrs: Mutex<Vec<Value>>,
    needs_resource_enrichment: AtomicBool,
    sent_event_keys: Mutex<VecDeque<String>>,
}

impl OtlpClient {
    pub fn new(endpoint: &str, headers: Vec<(String, String)>, data: &ScrapedData) -> Self {
        Self::with_resource_attrs(
            endpoint,
            headers,
            build_resource_attrs(Some(data), None),
            false,
        )
    }

    pub fn new_fallback(endpoint: &str, headers: Vec<(String, String)>, modem_url: &str) -> Self {
        Self::with_resource_attrs(
            endpoint,
            headers,
            build_resource_attrs(None, Some(modem_url)),
            true,
        )
    }

    fn with_resource_attrs(
        endpoint: &str,
        headers: Vec<(String, String)>,
        resource_attrs: Vec<Value>,
        needs_resource_enrichment: bool,
    ) -> Self {
        Self {
            client: Client::new(),
            metrics_endpoint: normalize_otlp_endpoint(endpoint, SignalKind::Metrics),
            logs_endpoint: normalize_otlp_endpoint(endpoint, SignalKind::Logs),
            headers,
            resource_attrs: Mutex::new(resource_attrs),
            needs_resource_enrichment: AtomicBool::new(needs_resource_enrichment),
            sent_event_keys: Mutex::new(VecDeque::with_capacity(256)),
        }
    }

    pub fn enrich_from_data(&self, data: &ScrapedData) {
        if self
            .needs_resource_enrichment
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let mut resource_attrs = self.resource_attrs.lock().unwrap();
            *resource_attrs = build_resource_attrs(Some(data), None);
        }
    }

    fn resource_attrs(&self) -> Vec<Value> {
        self.resource_attrs.lock().unwrap().clone()
    }

    pub async fn push(&self, data: &ScrapedData) -> Result<()> {
        self.enrich_from_data(data);
        self.push_metrics(data).await?;
        self.push_logs(data).await?;
        Ok(())
    }

    async fn push_metrics(&self, data: &ScrapedData) -> Result<()> {
        let body = self.build_metrics_body(data, &now_nanos());
        self.post_json(&self.metrics_endpoint, &body).await
    }

    fn build_metrics_body(&self, data: &ScrapedData, now: &str) -> Value {
        let boot_nanos = boot_time_nanos(data.uptime_seconds, now);
        let counter_start = boot_nanos.as_deref().unwrap_or(now);
        let mut metrics: Vec<Value> = Vec::new();

        metrics.push(gauge(
            "cm3500.exporter.status",
            "Whether the modem scrape was successful",
            "1",
            vec![f64_dp(1.0, vec![], now)],
        ));
        metrics.push(gauge(
            "cm3500.exporter.scrape.duration",
            "Duration of the last modem scrape",
            "s",
            vec![f64_dp(data.scrape_duration_secs, vec![], now)],
        ));

        if let Some(uptime) = data.uptime_seconds {
            metrics.push(sum(
                "system.uptime",
                "System uptime",
                "s",
                vec![f64_dp(uptime as f64, vec![], now)],
                counter_start,
                true,
            ));
        }

        if !data.cm_status.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.docsis.status",
                "Cable modem DOCSIS operational status",
                "1",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("docsis.state", &data.cm_status)],
                    now,
                )],
            ));
        }

        for iface in &data.interfaces {
            let up = if iface.state == "Up" { 1.0 } else { 0.0 };
            metrics.push(gauge(
                "system.network.interface.status",
                "Network interface operational status",
                "1",
                vec![f64_dp(
                    up,
                    vec![
                        str_attr("network.interface.name", &iface.name),
                        str_attr("network.interface.provisioned", &iface.provisioned),
                    ],
                    now,
                )],
            ));
        }

        metrics.push(gauge(
            "hw.cable_modem.cpe.detected",
            "Number of detected CPE devices",
            "1",
            vec![
                f64_dp(
                    data.cpe_static as f64,
                    vec![str_attr("cpe.kind", "static")],
                    now,
                ),
                f64_dp(
                    data.cpe_dynamic as f64,
                    vec![str_attr("cpe.kind", "dynamic")],
                    now,
                ),
            ],
        ));

        if !data.dhcp_state.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.provisioning.dhcp.status",
                "DHCP state of the cable modem",
                "1",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("dhcp.state", &data.dhcp_state)],
                    now,
                )],
            ));
        }
        for (name, desc, opt_val) in [
            (
                "hw.cable_modem.provisioning.dhcp.lease.duration",
                "Total DHCP lease duration",
                data.dhcp_info.lease_total_secs,
            ),
            (
                "hw.cable_modem.provisioning.dhcp.lease.remaining",
                "DHCP lease time remaining",
                data.dhcp_info.lease_remaining_secs,
            ),
            (
                "hw.cable_modem.provisioning.dhcp.rebind.remaining",
                "DHCP rebind time remaining",
                data.dhcp_info.rebind_remaining_secs,
            ),
            (
                "hw.cable_modem.provisioning.dhcp.renew.remaining",
                "DHCP renew time remaining",
                data.dhcp_info.renew_remaining_secs,
            ),
        ] {
            if let Some(v) = opt_val {
                metrics.push(gauge(name, desc, "s", vec![f64_dp(v as f64, vec![], now)]));
            }
        }

        let mut ds_power = Vec::new();
        let mut ds_snr = Vec::new();
        let mut ds_octets = Vec::new();
        let mut ds_corrected = Vec::new();
        let mut ds_uncorrectable = Vec::new();

        for ch in &data.downstream_qam {
            let attrs = downstream_channel_attrs(ch.channel, ch.dcid, ch.freq_mhz, &ch.modulation);
            ds_power.push(f64_dp(ch.power_dbmv, attrs.clone(), now));
            ds_snr.push(f64_dp(ch.snr_db, attrs.clone(), now));
            ds_octets.push(i64_dp(ch.octets as i64, attrs.clone(), now));
            ds_corrected.push(i64_dp(ch.correcteds as i64, attrs.clone(), now));
            ds_uncorrectable.push(i64_dp(ch.uncorrectables as i64, attrs, now));
        }

        if !ds_power.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.downstream.power",
                "Downstream SC-QAM power level",
                "dBmV",
                ds_power,
            ));
            metrics.push(gauge(
                "hw.cable_modem.downstream.snr",
                "Downstream SC-QAM signal-to-noise ratio",
                "dB",
                ds_snr,
            ));
            metrics.push(sum(
                "hw.cable_modem.downstream.octets",
                "Received octets on downstream SC-QAM channels",
                "By",
                ds_octets,
                counter_start,
                true,
            ));
            metrics.push(sum(
                "hw.cable_modem.downstream.errors.corrected.count",
                "Corrected downstream SC-QAM errors",
                "1",
                ds_corrected,
                counter_start,
                true,
            ));
            metrics.push(sum(
                "hw.cable_modem.downstream.errors.uncorrectable.count",
                "Uncorrectable downstream SC-QAM errors",
                "1",
                ds_uncorrectable,
                counter_start,
                true,
            ));
        }

        let mut ds_ofdm_width = Vec::new();
        let mut ds_ofdm_subcarriers = Vec::new();
        let mut ds_ofdm_rxmer = Vec::new();
        for ch in &data.downstream_ofdm {
            let base_attrs = vec![
                int_attr("cable.channel.id", ch.channel as i64),
                str_attr("cable.channel.kind", "ofdm"),
                str_attr("cable.ofdm.fft_type", &ch.fft_type),
            ];
            ds_ofdm_width.push(f64_dp(
                ch.channel_width_mhz * 1_000_000.0,
                base_attrs.clone(),
                now,
            ));
            ds_ofdm_subcarriers.push(f64_dp(
                ch.active_subcarriers as f64,
                base_attrs.clone(),
                now,
            ));
            for (measurement, val) in [
                ("pilot", ch.rxmer_pilot_db),
                ("plc", ch.rxmer_plc_db),
                ("data", ch.rxmer_data_db),
            ] {
                let mut attrs = base_attrs.clone();
                attrs.push(str_attr("cable.ofdm.measurement", measurement));
                ds_ofdm_rxmer.push(f64_dp(val, attrs, now));
            }
        }
        if !ds_ofdm_width.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.downstream.ofdm.channel.width",
                "Downstream OFDM channel width",
                "Hz",
                ds_ofdm_width,
            ));
            metrics.push(gauge(
                "hw.cable_modem.downstream.ofdm.subcarriers.active",
                "Active downstream OFDM subcarriers",
                "1",
                ds_ofdm_subcarriers,
            ));
            metrics.push(gauge(
                "hw.cable_modem.downstream.ofdm.rx_mer",
                "Downstream OFDM receive modulation error ratio",
                "dB",
                ds_ofdm_rxmer,
            ));
        }

        let mut us_power = Vec::new();
        let mut us_sym_rate = Vec::new();
        for ch in &data.upstream_qam {
            let attrs = vec![
                int_attr("cable.channel.id", ch.channel as i64),
                int_attr("cable.channel.ucid", ch.ucid as i64),
                int_attr("cable.channel.frequency", mhz_to_hz_i64(ch.freq_mhz)),
                str_attr("cable.channel.modulation", &ch.modulation),
                str_attr("cable.channel.kind", "sc_qam"),
                str_attr("cable.channel.type", &ch.channel_type),
            ];
            us_power.push(f64_dp(ch.power_dbmv, attrs.clone(), now));
            us_sym_rate.push(f64_dp(ch.symbol_rate_ksym * 1000.0, attrs, now));
        }
        if !us_power.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.upstream.power",
                "Upstream SC-QAM transmit power",
                "dBmV",
                us_power,
            ));
            metrics.push(gauge(
                "hw.cable_modem.upstream.symbol_rate",
                "Upstream SC-QAM symbol rate",
                "1/s",
                us_sym_rate,
            ));
        }

        let mut us_ofdma_power = Vec::new();
        let mut us_ofdma_width = Vec::new();
        let mut us_ofdma_subcarriers = Vec::new();
        let mut us_ofdma_subcarrier_range = Vec::new();
        for ch in &data.upstream_ofdm {
            let attrs = vec![
                int_attr("cable.channel.id", ch.channel as i64),
                str_attr("cable.channel.kind", "ofdma"),
                str_attr("cable.ofdma.fft_type", &ch.fft_type),
            ];
            us_ofdma_power.push(f64_dp(ch.tx_power_dbmv, attrs.clone(), now));
            us_ofdma_width.push(f64_dp(
                ch.channel_width_mhz * 1_000_000.0,
                attrs.clone(),
                now,
            ));
            us_ofdma_subcarriers.push(f64_dp(ch.active_subcarriers as f64, attrs.clone(), now));
            for (edge, value) in [
                ("first", ch.first_active_subcarrier_mhz),
                ("last", ch.last_active_subcarrier_mhz),
            ] {
                let mut edge_attrs = attrs.clone();
                edge_attrs.push(str_attr("cable.ofdma.subcarrier.edge", edge));
                us_ofdma_subcarrier_range.push(f64_dp(value * 1_000_000.0, edge_attrs, now));
            }
        }
        if !us_ofdma_power.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.upstream.power",
                "Upstream transmit power",
                "dBmV",
                us_ofdma_power,
            ));
            metrics.push(gauge(
                "hw.cable_modem.upstream.ofdma.channel.width",
                "Upstream OFDMA channel width",
                "Hz",
                us_ofdma_width,
            ));
            metrics.push(gauge(
                "hw.cable_modem.upstream.ofdma.subcarriers.active",
                "Active upstream OFDMA subcarriers",
                "1",
                us_ofdma_subcarriers,
            ));
            metrics.push(gauge(
                "hw.cable_modem.upstream.ofdma.subcarrier.range",
                "First and last active upstream OFDMA subcarrier positions",
                "Hz",
                us_ofdma_subcarrier_range,
            ));
        }

        if !data.qos_flows.is_empty() {
            let qos_dps: Vec<Value> = data
                .qos_flows
                .iter()
                .map(|f| {
                    i64_dp(
                        f.packets as i64,
                        vec![
                            int_attr("cable.qos.sfid", f.sfid as i64),
                            str_attr("cable.qos.direction", &f.direction),
                            str_attr(
                                "cable.qos.primary",
                                if f.primary { "true" } else { "false" },
                            ),
                            str_attr("cable.qos.service_class", &f.service_class),
                        ],
                        now,
                    )
                })
                .collect();
            metrics.push(sum(
                "hw.cable_modem.qos.packets.count",
                "QoS service flow packets",
                "1",
                qos_dps,
                counter_start,
                true,
            ));
        }

        if !data.cm_state.overall_state.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.docsis.registration.status",
                "Overall DOCSIS registration state",
                "1",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("docsis.state", &data.cm_state.overall_state)],
                    now,
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
                            str_attr("docsis.phase", &phase_normalized),
                            str_attr("docsis.phase.status", &p.status),
                        ],
                        now,
                    )
                })
                .collect();
            metrics.push(gauge(
                "hw.cable_modem.docsis.phase.status",
                "DOCSIS registration phase status",
                "1",
                phase_dps,
            ));
        }
        if !data.cm_state.bpi_status.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.docsis.bpi.status",
                "Baseline Privacy Interface state",
                "1",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("docsis.bpi.status", &data.cm_state.bpi_status)],
                    now,
                )],
            ));
        }
        if !data.cm_state.tod_status.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.docsis.tod.status",
                "Time of Day acquisition state",
                "1",
                vec![f64_dp(
                    1.0,
                    vec![str_attr("docsis.tod.status", &data.cm_state.tod_status)],
                    now,
                )],
            ));
        }

        if !data.events.is_empty() {
            let mut counts: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
            for e in &data.events {
                *counts.entry(&e.event_id).or_insert(0) += 1;
            }
            let event_dps: Vec<Value> = counts
                .iter()
                .map(|(id, count)| {
                    f64_dp(
                        *count as f64,
                        vec![str_attr("cable_modem.event.id", id)],
                        now,
                    )
                })
                .collect();
            metrics.push(gauge(
                "hw.cable_modem.event_log.entries",
                "Number of entries in the modem event log by event ID",
                "1",
                event_dps,
            ));

            let mut t3 = 0u32;
            let mut t4 = 0u32;
            let mut dhcp_fail = 0u32;
            let mut dhcp_warn = 0u32;
            let mut dhcp_no_response = 0u32;
            let mut profile_chg = 0u32;
            let mut us_profiles: std::collections::HashMap<String, u32> =
                std::collections::HashMap::new();
            let mut ofdma_profiles: std::collections::HashMap<String, u32> =
                std::collections::HashMap::new();
            for e in &data.events {
                match e.event_id.as_str() {
                    "82000200" => t3 += 1,
                    "82000300" => t4 += 1,
                    "68001202" => dhcp_fail += 1,
                    "68010300" => dhcp_warn += 1,
                    "68010100" => dhcp_no_response += 1,
                    "67061601" => profile_chg += 1,
                    _ => {}
                }
                if let Some((chan, profile)) = extract_us_profile(&e.description) {
                    us_profiles.insert(chan, profile);
                }
                if let Some((chan, profile)) = extract_ofdma_profile(&e.description) {
                    ofdma_profiles.insert(chan, profile);
                }
            }
            for (name, desc, val) in [
                (
                    "hw.cable_modem.cmts.t3_timeouts.count",
                    "T3 timeouts in the modem event log",
                    t3,
                ),
                (
                    "hw.cable_modem.cmts.t4_timeouts.count",
                    "T4 timeouts in the modem event log",
                    t4,
                ),
                (
                    "hw.cable_modem.provisioning.dhcp_failures.count",
                    "DHCP failures in the modem event log",
                    dhcp_fail,
                ),
                (
                    "hw.cable_modem.provisioning.dhcp_renew_warnings.count",
                    "DHCP renew warnings in the modem event log",
                    dhcp_warn,
                ),
                (
                    "hw.cable_modem.provisioning.dhcp_renew_no_response.count",
                    "DHCP renew no response events in the modem event log",
                    dhcp_no_response,
                ),
                (
                    "hw.cable_modem.upstream.profile_changes.count",
                    "Upstream profile assignment changes in the modem event log",
                    profile_chg,
                ),
            ] {
                metrics.push(gauge(
                    name,
                    desc,
                    "1",
                    vec![f64_dp(val as f64, vec![], now)],
                ));
            }

            if !us_profiles.is_empty() {
                let mut dps = Vec::new();
                let mut items = us_profiles.into_iter().collect::<Vec<_>>();
                items.sort();
                for (channel, profile) in items {
                    dps.push(f64_dp(
                        profile as f64,
                        vec![int_attr(
                            "cable.channel.id",
                            channel.parse::<u32>().unwrap_or_default() as i64,
                        )],
                        now,
                    ));
                }
                metrics.push(gauge(
                    "hw.cable_modem.upstream.profile.active",
                    "Last seen upstream profile ID from modem event log",
                    "1",
                    dps,
                ));
            }

            if !ofdma_profiles.is_empty() {
                let mut dps = Vec::new();
                let mut items = ofdma_profiles.into_iter().collect::<Vec<_>>();
                items.sort();
                for (channel, profile) in items {
                    dps.push(f64_dp(
                        profile as f64,
                        vec![int_attr(
                            "cable.channel.id",
                            channel.parse::<u32>().unwrap_or_default() as i64,
                        )],
                        now,
                    ));
                }
                metrics.push(gauge(
                    "hw.cable_modem.upstream.ofdma.profile.active",
                    "Last seen OFDM/OFDMA profile ID from modem event log",
                    "1",
                    dps,
                ));
            }
        }

        if !data.product_info.ethernet_phy_type.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.ethernet.phy.info",
                "Ethernet PHY type reported by the modem",
                "1",
                vec![f64_dp(
                    1.0,
                    vec![str_attr(
                        "network.ethernet.phy.type",
                        &data.product_info.ethernet_phy_type,
                    )],
                    now,
                )],
            ));
        }
        if !data.product_info.logging_components_enabled.is_empty() {
            let dps: Vec<Value> = data
                .product_info
                .logging_components_enabled
                .iter()
                .map(|(group, count)| {
                    f64_dp(
                        *count as f64,
                        vec![str_attr("cm3500.product.logging.group", group)],
                        now,
                    )
                })
                .collect();
            metrics.push(gauge(
                "hw.cable_modem.product.logging_components.enabled",
                "Number of enabled internal logging components by product debug group",
                "1",
                dps,
            ));
        }
        if !data.spectrum.is_empty() {
            metrics.push(gauge(
                "hw.cable_modem.spectrum.chunks",
                "Number of spectrum scan chunks returned by the modem",
                "1",
                vec![f64_dp(data.spectrum.len() as f64, vec![], now)],
            ));
            let mut chunk_power = Vec::new();
            let mut chunk_meta = Vec::new();
            let mut bin_power = Vec::new();
            for chunk in &data.spectrum {
                if chunk.bins_raw_tenth_dbmv.is_empty() {
                    continue;
                }
                let min_raw = *chunk.bins_raw_tenth_dbmv.iter().min().unwrap() as f64;
                let max_raw = *chunk.bins_raw_tenth_dbmv.iter().max().unwrap() as f64;
                let avg_raw = chunk
                    .bins_raw_tenth_dbmv
                    .iter()
                    .map(|v| *v as f64)
                    .sum::<f64>()
                    / chunk.bins_raw_tenth_dbmv.len() as f64;
                let base_attrs = vec![
                    int_attr("cm3500.spectrum.chunk", chunk.chunk as i64),
                    int_attr("cable.spectrum.center_frequency", chunk.center_freq_hz),
                ];
                for (stat, value) in [
                    ("min", min_raw / 10.0),
                    ("avg", avg_raw / 10.0),
                    ("max", max_raw / 10.0),
                ] {
                    let mut attrs = base_attrs.clone();
                    attrs.push(str_attr("cable.spectrum.stat", stat));
                    chunk_power.push(f64_dp(value, attrs, now));
                }
                for (kind, value) in [
                    ("span_hz", chunk.span_hz as f64),
                    ("bin_spacing_hz", chunk.bin_spacing_hz as f64),
                    ("resolution_bandwidth", chunk.resolution_bandwidth as f64),
                ] {
                    let mut attrs = base_attrs.clone();
                    attrs.push(str_attr("cable.spectrum.metric", kind));
                    chunk_meta.push(f64_dp(value, attrs, now));
                }
                let start_hz = chunk.center_freq_hz - (chunk.span_hz / 2);
                for (idx, raw) in chunk.bins_raw_tenth_dbmv.iter().enumerate() {
                    bin_power.push(f64_dp(
                        *raw as f64 / 10.0,
                        vec![
                            int_attr("cm3500.spectrum.chunk", chunk.chunk as i64),
                            int_attr("cable.spectrum.bin", idx as i64),
                            int_attr(
                                "cable.spectrum.frequency",
                                start_hz + (idx as i64 * chunk.bin_spacing_hz),
                            ),
                        ],
                        now,
                    ));
                }
            }
            metrics.push(gauge(
                "hw.cable_modem.spectrum.chunk.power",
                "Spectrum scan chunk power summary",
                "dBmV",
                chunk_power,
            ));
            metrics.push(gauge(
                "hw.cable_modem.spectrum.chunk.metadata",
                "Spectrum scan chunk metadata",
                "1",
                chunk_meta,
            ));
            metrics.push(gauge(
                "hw.cable_modem.spectrum.bin.power",
                "Spectrum scan bin power",
                "dBmV",
                bin_power,
            ));
        }

        if !data.service_flow_configs.is_empty() {
            let max_rate: Vec<Value> = data
                .service_flow_configs
                .iter()
                .map(|f| {
                    f64_dp(
                        f.max_traffic_rate_kbps as f64 * 1000.0,
                        vec![
                            str_attr("cable.qos.direction", &f.direction),
                            int_attr("cable.qos.flow_index", f.index as i64),
                        ],
                        now,
                    )
                })
                .collect();
            let min_rate: Vec<Value> = data
                .service_flow_configs
                .iter()
                .map(|f| {
                    f64_dp(
                        f.min_reserved_rate_kbps as f64 * 1000.0,
                        vec![
                            str_attr("cable.qos.direction", &f.direction),
                            int_attr("cable.qos.flow_index", f.index as i64),
                        ],
                        now,
                    )
                })
                .collect();
            let max_burst: Vec<Value> = data
                .service_flow_configs
                .iter()
                .map(|f| {
                    f64_dp(
                        f.max_traffic_burst as f64,
                        vec![
                            str_attr("cable.qos.direction", &f.direction),
                            int_attr("cable.qos.flow_index", f.index as i64),
                        ],
                        now,
                    )
                })
                .collect();
            let priority: Vec<Value> = data
                .service_flow_configs
                .iter()
                .map(|f| {
                    f64_dp(
                        f.traffic_priority as f64,
                        vec![
                            str_attr("cable.qos.direction", &f.direction),
                            int_attr("cable.qos.flow_index", f.index as i64),
                        ],
                        now,
                    )
                })
                .collect();
            metrics.push(gauge(
                "hw.cable_modem.qos.max_traffic.rate",
                "Maximum QoS service flow traffic rate",
                "bit/s",
                max_rate,
            ));
            metrics.push(gauge(
                "hw.cable_modem.qos.min_reserved.rate",
                "Minimum reserved QoS service flow rate",
                "bit/s",
                min_rate,
            ));
            metrics.push(gauge(
                "hw.cable_modem.qos.max_traffic.burst",
                "Maximum QoS service flow traffic burst",
                "By",
                max_burst,
            ));
            metrics.push(gauge(
                "hw.cable_modem.qos.traffic.priority",
                "QoS service flow traffic priority",
                "1",
                priority,
            ));
        }

        json!({
            "resourceMetrics": [{
                "resource": { "attributes": self.resource_attrs() },
                "scopeMetrics": [{
                    "scope": {
                        "name": "cm3500-exporter",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "metrics": metrics
                }]
            }]
        })
    }

    async fn push_logs(&self, data: &ScrapedData) -> Result<()> {
        let now = now_nanos();
        let Some(body) = self.build_logs_body(data, &now) else {
            return Ok(());
        };
        self.post_json(&self.logs_endpoint, &body).await
    }

    fn build_logs_body(&self, data: &ScrapedData, now: &str) -> Option<Value> {
        let records = self.new_log_records(&data.events, now);
        if records.is_empty() {
            return None;
        }

        Some(json!({
            "resourceLogs": [{
                "resource": { "attributes": self.resource_attrs() },
                "scopeLogs": [{
                    "scope": {
                        "name": "cm3500-exporter",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "logRecords": records
                }]
            }]
        }))
    }

    fn new_log_records(&self, events: &[EventLogEntry], now: &str) -> Vec<Value> {
        let mut sent = self.sent_event_keys.lock().unwrap();
        let mut records = Vec::new();

        for event in events {
            let key = format!(
                "{}|{}|{}|{}",
                event.timestamp, event.event_id, event.event_level, event.description
            );
            if sent.iter().any(|existing| existing == &key) {
                continue;
            }

            let (event_type, event_name) = event_kind(&event.event_id);
            let (severity_number, severity_text) = severity_for_level(event.event_level);
            let mut attrs = vec![
                str_attr("event.name", event_name),
                str_attr("event.domain", "cable_modem"),
                str_attr("cable_modem.event.id", &event.event_id),
                str_attr("cable_modem.event.type", event_type),
                int_attr("cable_modem.event.level", event.event_level as i64),
                str_attr("cable_modem.event.timestamp", &event.timestamp),
            ];
            if let Some(cm_mac) = extract_suffix_value(&event.description, "CM-MAC=") {
                attrs.push(str_attr("cable_modem.cm.mac_address", &cm_mac));
            }
            if let Some(cmts_mac) = extract_suffix_value(&event.description, "CMTS-MAC=") {
                attrs.push(str_attr("cable_modem.cmts.mac_address", &cmts_mac));
            }

            records.push(json!({
                "timeUnixNano": now,
                "observedTimeUnixNano": now,
                "severityNumber": severity_number,
                "severityText": severity_text,
                "body": { "stringValue": event.description },
                "attributes": attrs,
            }));

            sent.push_back(key);
            while sent.len() > 256 {
                sent.pop_front();
            }
        }

        records
    }

    async fn post_json(&self, endpoint: &str, body: &Value) -> Result<()> {
        let mut req = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(body);

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

#[derive(Copy, Clone)]
enum SignalKind {
    Metrics,
    Logs,
}

fn normalize_otlp_endpoint(endpoint: &str, kind: SignalKind) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    let suffix = match kind {
        SignalKind::Metrics => "/v1/metrics",
        SignalKind::Logs => "/v1/logs",
    };

    if trimmed.ends_with("/v1/metrics") {
        return match kind {
            SignalKind::Metrics => trimmed.to_string(),
            SignalKind::Logs => trimmed.trim_end_matches("/metrics").to_string() + "/logs",
        };
    }
    if trimmed.ends_with("/v1/logs") {
        return match kind {
            SignalKind::Logs => trimmed.to_string(),
            SignalKind::Metrics => trimmed.trim_end_matches("/logs").to_string() + "/metrics",
        };
    }
    format!("{trimmed}{suffix}")
}

fn now_nanos() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

fn boot_time_nanos(uptime_seconds: Option<u64>, now_nanos: &str) -> Option<String> {
    let uptime_seconds = uptime_seconds?;
    let now: u128 = now_nanos.parse().ok()?;
    let offset = uptime_seconds as u128 * 1_000_000_000;
    Some(now.saturating_sub(offset).to_string())
}

fn downstream_channel_attrs(
    channel: u32,
    dcid: u32,
    freq_mhz: f64,
    modulation: &str,
) -> Vec<Value> {
    vec![
        int_attr("cable.channel.id", channel as i64),
        int_attr("cable.channel.dcid", dcid as i64),
        int_attr("cable.channel.frequency", mhz_to_hz_i64(freq_mhz)),
        str_attr("cable.channel.modulation", modulation),
        str_attr("cable.channel.kind", "sc_qam"),
    ]
}

fn mhz_to_hz_i64(freq_mhz: f64) -> i64 {
    (freq_mhz * 1_000_000.0).round() as i64
}

fn str_attr(key: &str, value: &str) -> Value {
    json!({"key": key, "value": {"stringValue": value}})
}

fn build_resource_attrs(data: Option<&ScrapedData>, modem_url: Option<&str>) -> Vec<Value> {
    if let Some(data) = data {
        let hw_device_id = data
            .interfaces
            .iter()
            .find(|iface| !iface.mac_address.is_empty())
            .map(|iface| iface.mac_address.as_str())
            .filter(|mac| *mac != "00:00:00:00:00:00")
            .unwrap_or(&data.version_info.serial);

        return vec![
            str_attr("service.name", "cm3500-exporter"),
            str_attr("service.version", env!("CARGO_PKG_VERSION")),
            str_attr("service.instance.id", &data.version_info.serial),
            str_attr("hw.device.vendor", &data.version_info.vendor),
            str_attr("hw.device.model", &data.version_info.model),
            str_attr("hw.device.id", hw_device_id),
            str_attr("network.connection.type", "cable"),
            str_attr("cm3500.firmware.version", &data.version_info.software_rev),
            str_attr("cm3500.firmware.name", &data.version_info.firmware_name),
        ];
    }

    vec![
        str_attr("service.name", "cm3500-exporter"),
        str_attr("service.version", env!("CARGO_PKG_VERSION")),
        str_attr("service.instance.id", modem_url.unwrap_or("unknown")),
        str_attr("hw.device.vendor", "ARRIS"),
        str_attr("hw.device.model", "CM3500B CE"),
        str_attr("hw.device.id", "unknown"),
        str_attr("network.connection.type", "cable"),
    ]
}

fn int_attr(key: &str, value: i64) -> Value {
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
        "as_int": value.to_string(),
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
    monotonic: bool,
) -> Value {
    let dps: Vec<Value> = data_points
        .into_iter()
        .map(|dp| with_start_time(dp, start_nanos))
        .collect();
    json!({
        "name": name,
        "description": description,
        "unit": unit,
        "sum": {
            "dataPoints": dps,
            "isMonotonic": monotonic,
            "aggregationTemporality": 2
        }
    })
}

fn with_start_time(dp: Value, start_nanos: &str) -> Value {
    let mut obj: Map<String, Value> = dp.as_object().cloned().unwrap_or_default();
    obj.insert(
        "startTimeUnixNano".to_string(),
        Value::String(start_nanos.to_string()),
    );
    Value::Object(obj)
}

fn event_kind(event_id: &str) -> (&'static str, &'static str) {
    match event_id {
        "82000200" => ("T3_TIMEOUT", "cable_modem.docsis_event"),
        "82000300" => ("T4_TIMEOUT", "cable_modem.docsis_event"),
        "68001202" => ("DHCP_FAILURE", "cable_modem.provisioning_event"),
        "68010300" => ("DHCP_RENEW_WARNING", "cable_modem.provisioning_event"),
        "67061601" => ("US_PROFILE_CHANGE", "cable_modem.docsis_event"),
        _ => ("MODEM_EVENT", "cable_modem.event"),
    }
}

fn severity_for_level(level: u32) -> (u32, &'static str) {
    match level {
        0 | 1 => (9, "INFO"),
        2 | 3 => (13, "WARN"),
        _ => (17, "ERROR"),
    }
}

fn extract_suffix_value(description: &str, prefix: &str) -> Option<String> {
    let start = description.find(prefix)? + prefix.len();
    let rest = &description[start..];
    let end = rest.find(';').unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn extract_us_profile(description: &str) -> Option<(String, u32)> {
    let channel = description
        .split("US Chan ID:")
        .nth(1)?
        .split(';')
        .next()?
        .trim();
    let profile = description
        .split("New Profile:")
        .nth(1)?
        .split('.')
        .next()?
        .trim();
    Some((channel.to_string(), profile.parse().ok()?))
}

fn extract_ofdma_profile(description: &str) -> Option<(String, u32)> {
    let channel = description
        .split("Chan ID:")
        .nth(1)?
        .split(';')
        .next()?
        .trim();
    let profile = description
        .split("OFDMA Profile ID:")
        .nth(1)?
        .split('.')
        .next()?
        .trim();
    Some((channel.to_string(), profile.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{
        CmState, DhcpInfo, DocsisPhase, DownstreamOfdm, DownstreamQam, EventLogEntry,
        InterfaceInfo, ProductInfo, QosFlow, ScrapedData, ServiceFlowConfig, SpectrumChunk,
        UpstreamOfdm, UpstreamQam, VersionInfo,
    };

    #[test]
    fn normalizes_otlp_endpoints() {
        assert_eq!(
            normalize_otlp_endpoint("https://example.test/otlp", SignalKind::Metrics),
            "https://example.test/otlp/v1/metrics"
        );
        assert_eq!(
            normalize_otlp_endpoint("https://example.test/otlp", SignalKind::Logs),
            "https://example.test/otlp/v1/logs"
        );
        assert_eq!(
            normalize_otlp_endpoint("https://example.test/v1/metrics", SignalKind::Logs),
            "https://example.test/v1/logs"
        );
    }

    #[test]
    fn extracts_event_fields() {
        let desc = "No Ranging Response; CM-MAC=aa:bb:cc:dd:ee:ff; CMTS-MAC=11:22:33:44:55:66;";
        assert_eq!(
            extract_suffix_value(desc, "CM-MAC=").as_deref(),
            Some("aa:bb:cc:dd:ee:ff")
        );
        assert_eq!(
            extract_suffix_value(desc, "CMTS-MAC=").as_deref(),
            Some("11:22:33:44:55:66")
        );
    }

    #[test]
    fn builds_otlp_metrics_payload_with_expected_names_and_resources() {
        let data = sample_data();
        let client = OtlpClient::new("https://example.test/otlp", vec![], &data);
        let body = client.build_metrics_body(&data, "1000000000");

        let resource_attrs = &body["resourceMetrics"][0]["resource"]["attributes"];
        assert!(has_attr(resource_attrs, "service.name", "cm3500-exporter"));
        assert!(has_attr(resource_attrs, "hw.device.vendor", "ARRIS"));
        assert!(has_attr(resource_attrs, "hw.device.model", "CM3500 B CE"));
        assert!(has_attr(
            resource_attrs,
            "hw.device.id",
            "aa:bb:cc:dd:ee:ff"
        ));
        assert!(has_attr(resource_attrs, "network.connection.type", "cable"));

        let metrics = body["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
            .as_array()
            .unwrap();
        assert!(has_metric(metrics, "cm3500.exporter.status"));
        assert!(has_metric(metrics, "system.uptime"));
        assert!(has_metric(metrics, "system.network.interface.status"));
        assert!(has_metric(metrics, "hw.cable_modem.downstream.power"));
        assert!(has_metric(metrics, "hw.cable_modem.downstream.ofdm.rx_mer"));
        assert!(has_metric(
            metrics,
            "hw.cable_modem.upstream.ofdma.subcarrier.range"
        ));
        assert!(has_metric(
            metrics,
            "hw.cable_modem.provisioning.dhcp_failures.count"
        ));
        assert!(has_metric(
            metrics,
            "hw.cable_modem.provisioning.dhcp_renew_no_response.count"
        ));
        assert!(has_metric(
            metrics,
            "hw.cable_modem.upstream.profile.active"
        ));
        assert!(has_metric(
            metrics,
            "hw.cable_modem.upstream.ofdma.profile.active"
        ));
        assert!(has_metric(metrics, "hw.cable_modem.ethernet.phy.info"));
        assert!(has_metric(
            metrics,
            "hw.cable_modem.product.logging_components.enabled"
        ));
        assert!(has_metric(metrics, "hw.cable_modem.spectrum.bin.power"));
        assert!(has_metric(metrics, "hw.cable_modem.qos.max_traffic.rate"));

        let rx_mer = metric(metrics, "hw.cable_modem.downstream.ofdm.rx_mer");
        let rx_mer_attrs = &rx_mer["gauge"]["dataPoints"][0]["attributes"];
        assert!(has_attr(rx_mer_attrs, "cable.ofdm.measurement", "pilot"));
    }

    #[test]
    fn builds_otlp_log_payload_and_deduplicates() {
        let data = sample_data();
        let client = OtlpClient::new("https://example.test/otlp", vec![], &data);

        let first = client.build_logs_body(&data, "1000000000").unwrap();
        let records = first["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
            .as_array()
            .unwrap();
        assert_eq!(records.len(), 5);
        let first_record = &records[0];
        assert_eq!(first_record["severityText"].as_str().unwrap(), "WARN");
        assert_eq!(
            attr_value(&first_record["attributes"], "event.name").as_deref(),
            Some("cable_modem.docsis_event")
        );
        assert_eq!(
            attr_value(&first_record["attributes"], "event.domain").as_deref(),
            Some("cable_modem")
        );
        assert_eq!(
            attr_value(&first_record["attributes"], "cable_modem.event.type").as_deref(),
            Some("T3_TIMEOUT")
        );
        assert_eq!(
            attr_value(&first_record["attributes"], "cable_modem.cmts.mac_address").as_deref(),
            Some("11:22:33:44:55:66")
        );

        let second = client.build_logs_body(&data, "1000000001");
        assert!(second.is_none());
    }

    #[test]
    fn otlp_metrics_fixture_snapshot() {
        let data = fixture_data();
        let client = OtlpClient::new("https://example.test/otlp", vec![], &data);
        let body = client.build_metrics_body(&data, "1000000000");
        let snapshot = metrics_snapshot_summary(&body);
        assert_eq!(
            snapshot,
            include_str!("../tests/fixtures/otlp_metrics_snapshot.txt")
        );
    }

    #[test]
    fn otlp_logs_fixture_snapshot() {
        let data = fixture_data();
        let client = OtlpClient::new("https://example.test/otlp", vec![], &data);
        let body = client.build_logs_body(&data, "1000000000").unwrap();
        let snapshot = logs_snapshot_summary(&body);
        assert_eq!(
            snapshot,
            include_str!("../tests/fixtures/otlp_logs_snapshot.txt")
        );
    }

    #[test]
    fn fallback_otlp_client_enriches_resource_attrs_after_successful_scrape() {
        let data = sample_data();
        let client =
            OtlpClient::new_fallback("https://example.test/otlp", vec![], "https://192.168.100.1");

        let before = client.build_metrics_body(&data, "1000000000");
        let before_attrs = &before["resourceMetrics"][0]["resource"]["attributes"];
        assert!(has_attr(before_attrs, "hw.device.id", "unknown"));

        client.enrich_from_data(&data);
        let after = client.build_metrics_body(&data, "1000000001");
        let after_attrs = &after["resourceMetrics"][0]["resource"]["attributes"];
        assert!(has_attr(after_attrs, "hw.device.id", "aa:bb:cc:dd:ee:ff"));
    }

    fn has_metric(metrics: &[Value], name: &str) -> bool {
        metrics.iter().any(|m| m["name"].as_str() == Some(name))
    }

    fn metric<'a>(metrics: &'a [Value], name: &str) -> &'a Value {
        metrics
            .iter()
            .find(|m| m["name"].as_str() == Some(name))
            .unwrap()
    }

    fn has_attr(attrs: &Value, key: &str, value: &str) -> bool {
        attr_value(attrs, key).as_deref() == Some(value)
    }

    fn attr_value(attrs: &Value, key: &str) -> Option<String> {
        attrs
            .as_array()?
            .iter()
            .find(|a| a["key"].as_str() == Some(key))
            .and_then(|a| {
                a["value"]["stringValue"]
                    .as_str()
                    .map(ToString::to_string)
                    .or_else(|| a["value"]["intValue"].as_str().map(ToString::to_string))
            })
    }

    fn fixture_data() -> ScrapedData {
        crate::parser::parse_all(
            include_str!("../tests/fixtures/status_cgi.html"),
            include_str!("../tests/fixtures/vers_cgi.html"),
            include_str!("../tests/fixtures/dhcp_cgi.html"),
            include_str!("../tests/fixtures/qos_cgi.html"),
            include_str!("../tests/fixtures/cm_state_cgi.html"),
            include_str!("../tests/fixtures/event_cgi.html"),
            include_str!("../tests/fixtures/config_params_cgi.html"),
            "",
            None,
            1.5,
        )
        .unwrap()
    }

    fn metrics_snapshot_summary(body: &Value) -> String {
        let resource_attrs = body["resourceMetrics"][0]["resource"]["attributes"]
            .as_array()
            .unwrap();
        let mut resource_lines = [
            format!(
                "service.name={}",
                attr_value_from_array(resource_attrs, "service.name").unwrap()
            ),
            format!(
                "hw.device.vendor={}",
                attr_value_from_array(resource_attrs, "hw.device.vendor").unwrap()
            ),
            format!(
                "hw.device.model={}",
                attr_value_from_array(resource_attrs, "hw.device.model").unwrap()
            ),
            format!(
                "network.connection.type={}",
                attr_value_from_array(resource_attrs, "network.connection.type").unwrap()
            ),
        ];
        resource_lines.sort();

        let mut metrics = body["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        metrics.sort();
        metrics.dedup();

        let rx_mer = metric(
            body["resourceMetrics"][0]["scopeMetrics"][0]["metrics"]
                .as_array()
                .unwrap(),
            "hw.cable_modem.downstream.ofdm.rx_mer",
        );
        let rx_mer_dp = &rx_mer["gauge"]["dataPoints"][0];
        let rx_mer_attrs = rx_mer_dp["attributes"].as_array().unwrap();
        let mut rx_mer_lines = rx_mer_attrs
            .iter()
            .filter_map(|a| {
                let key = a["key"].as_str()?;
                if [
                    "cable.channel.id",
                    "cable.channel.kind",
                    "cable.ofdm.fft_type",
                    "cable.ofdm.measurement",
                ]
                .contains(&key)
                {
                    Some(format!(
                        "{}={}",
                        key,
                        attr_value_from_entry(a).unwrap_or_default()
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        rx_mer_lines.sort();

        format!(
            "[resource]\n{}\n\n[metric_names]\n{}\n\n[rx_mer_first_point]\n{}\n",
            resource_lines.join("\n"),
            metrics.join("\n"),
            rx_mer_lines.join("\n")
        )
    }

    fn logs_snapshot_summary(body: &Value) -> String {
        let mut lines = Vec::new();
        let records = body["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
            .as_array()
            .unwrap();
        for record in records {
            lines.push(format!(
                "severity={} event.name={} event.type={} event.id={} body={}",
                record["severityText"].as_str().unwrap(),
                attr_value(&record["attributes"], "event.name").unwrap(),
                attr_value(&record["attributes"], "cable_modem.event.type").unwrap(),
                attr_value(&record["attributes"], "cable_modem.event.id").unwrap(),
                record["body"]["stringValue"].as_str().unwrap(),
            ));
        }
        lines.sort();
        lines.join("\n") + "\n"
    }

    fn attr_value_from_array(attrs: &[Value], key: &str) -> Option<String> {
        attrs
            .iter()
            .find(|a| a["key"].as_str() == Some(key))
            .and_then(attr_value_from_entry)
    }

    fn attr_value_from_entry(a: &Value) -> Option<String> {
        a["value"]["stringValue"]
            .as_str()
            .map(ToString::to_string)
            .or_else(|| a["value"]["intValue"].as_str().map(ToString::to_string))
    }

    fn sample_data() -> ScrapedData {
        ScrapedData {
            downstream_qam: vec![DownstreamQam {
                channel: 1,
                dcid: 3,
                freq_mhz: 570.0,
                power_dbmv: 2.2,
                snr_db: 40.37,
                modulation: "256QAM".into(),
                octets: 2858,
                correcteds: 12,
                uncorrectables: 0,
            }],
            downstream_ofdm: vec![DownstreamOfdm {
                channel: 33,
                fft_type: "4K".into(),
                channel_width_mhz: 192.0,
                active_subcarriers: 3800,
                rxmer_pilot_db: 38.1,
                rxmer_plc_db: 37.9,
                rxmer_data_db: 37.4,
            }],
            upstream_qam: vec![UpstreamQam {
                channel: 5,
                ucid: 7,
                freq_mhz: 31.2,
                power_dbmv: 44.5,
                channel_type: "ATDMA".into(),
                symbol_rate_ksym: 5120.0,
                modulation: "64QAM".into(),
            }],
            upstream_ofdm: vec![UpstreamOfdm {
                channel: 9,
                fft_type: "4K".into(),
                channel_width_mhz: 96.0,
                active_subcarriers: 1800,
                first_active_subcarrier_mhz: 74.0,
                last_active_subcarrier_mhz: 170.0,
                tx_power_dbmv: 43.0,
            }],
            uptime_seconds: Some(3600),
            cm_status: "OPERATIONAL".into(),
            version_info: VersionInfo {
                hardware_rev: "1".into(),
                vendor: "ARRIS".into(),
                bootloader: "boot".into(),
                software_rev: "9.1.103AA65L".into(),
                model: "CM3500 B CE".into(),
                serial: "SERIAL123".into(),
                firmware_name: "firmware.bin".into(),
                firmware_build_time: "now".into(),
            },
            dhcp_info: DhcpInfo {
                cm_ip: "192.0.2.10".into(),
                cm_subnet: "255.255.255.0".into(),
                cm_gateway: "192.0.2.1".into(),
                lease_total_secs: Some(600),
                lease_remaining_secs: Some(303),
                rebind_total_secs: Some(500),
                rebind_remaining_secs: Some(250),
                renew_total_secs: Some(400),
                renew_remaining_secs: Some(200),
            },
            qos_flows: vec![QosFlow {
                sfid: 123,
                service_class: "BE".into(),
                direction: "Upstream".into(),
                primary: true,
                packets: 42,
            }],
            cm_state: CmState {
                overall_state: "Operational".into(),
                phases: vec![DocsisPhase {
                    phase: "Docsis-Config-File".into(),
                    status: "OK".into(),
                }],
                bpi_status: "Authorized".into(),
                tod_status: "Acquired".into(),
            },
            events: vec![
                EventLogEntry {
                    timestamp: "6/2/2026 7:11".into(),
                    event_id: "82000200".into(),
                    event_level: 3,
                    description: "No Ranging Response received - T3 time-out; CM-MAC=aa:bb:cc:dd:ee:ff; CMTS-MAC=11:22:33:44:55:66;".into(),
                },
                EventLogEntry {
                    timestamp: "6/2/2026 7:12".into(),
                    event_id: "68001202".into(),
                    event_level: 4,
                    description: "DHCP failed - Solicit sent, No Advertise received".into(),
                },
                EventLogEntry {
                    timestamp: "6/2/2026 7:13".into(),
                    event_id: "68010100".into(),
                    event_level: 4,
                    description: "DHCP RENEW sent - No response for IPv4".into(),
                },
                EventLogEntry {
                    timestamp: "6/2/2026 7:14".into(),
                    event_id: "67061601".into(),
                    event_level: 6,
                    description: "US profile assignment change.  US Chan ID: 10; Previous Profile: 9; New Profile: 12.;CM-MAC=aa:bb:cc:dd:ee:ff;CMTS-MAC=11:22:33:44:55:66;".into(),
                },
                EventLogEntry {
                    timestamp: "6/2/2026 7:15".into(),
                    event_id: "74010100".into(),
                    event_level: 6,
                    description: "CM-STATUS message sent.  Event Type Code: 16; Chan ID: 33; DSID: N/A; MAC Addr: N/A; OFDM/OFDMA Profile ID: 2.;CM-MAC=aa:bb:cc:dd:ee:ff;CMTS-MAC=11:22:33:44:55:66;".into(),
                },
            ],
            interfaces: vec![InterfaceInfo {
                name: "WAN0".into(),
                provisioned: "Yes".into(),
                state: "Up".into(),
                mac_address: "aa:bb:cc:dd:ee:ff".into(),
            }],
            product_info: ProductInfo {
                ethernet_phy_type: "1x2.5G-GPY21X switch".into(),
                logging_components_enabled: vec![("COMMON_COMPONENTS".into(), 2)],
            },
            spectrum: vec![SpectrumChunk {
                chunk: 0,
                center_freq_hz: 273_000_000,
                span_hz: 7_500_000,
                bin_spacing_hz: 117_187,
                resolution_bandwidth: 1,
                bins_raw_tenth_dbmv: vec![-1500, -1400, -1300],
            }],
            cpe_static: 0,
            cpe_dynamic: 1,
            dhcp_state: "bound".into(),
            service_flow_configs: vec![ServiceFlowConfig {
                direction: "Downstream".into(),
                index: 0,
                max_traffic_rate_kbps: 1126400,
                max_traffic_burst: 3044,
                min_reserved_rate_kbps: 0,
                traffic_priority: 1,
                scheduling_type: 2,
            }],
            scrape_duration_secs: 1.23,
        }
    }
}
