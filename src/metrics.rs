use crate::parser::ScrapedData;

pub fn render_metrics(data: &ScrapedData) -> String {
    let mut out = String::with_capacity(8192);

    // Scrape metrics
    metric(
        &mut out,
        "cm3500_up",
        "Whether the modem scrape was successful",
        "gauge",
        "1",
    );
    metric_f(
        &mut out,
        "cm3500_scrape_duration_seconds",
        "Duration of the last scrape in seconds",
        "gauge",
        data.scrape_duration_secs,
    );

    // Info metric
    labeled_metric(
        &mut out,
        "cm3500_info",
        "Modem hardware and firmware information",
        "gauge",
        &[
            ("model", &data.version_info.model),
            ("serial", &data.version_info.serial),
            ("firmware", &data.version_info.software_rev),
            ("firmware_name", &data.version_info.firmware_name),
            ("bootloader", &data.version_info.bootloader),
            ("hardware_rev", &data.version_info.hardware_rev),
            ("vendor", &data.version_info.vendor),
        ],
        "1",
    );

    // Uptime
    if let Some(uptime) = data.uptime_seconds {
        metric(
            &mut out,
            "cm3500_uptime_seconds",
            "System uptime in seconds",
            "gauge",
            &uptime.to_string(),
        );
    }

    // CM Status
    if !data.cm_status.is_empty() {
        labeled_metric(
            &mut out,
            "cm3500_cm_status",
            "Cable modem operational status",
            "gauge",
            &[("status", &data.cm_status)],
            "1",
        );
    }

    render_downstream_qam(&mut out, &data.downstream_qam);
    render_downstream_ofdm(&mut out, &data.downstream_ofdm);
    render_upstream_qam(&mut out, &data.upstream_qam);
    render_upstream_ofdm(&mut out, &data.upstream_ofdm);
    render_dhcp(&mut out, &data.dhcp_info, &data.dhcp_state);
    render_qos(&mut out, &data.qos_flows);
    render_cm_state(&mut out, &data.cm_state);
    render_event_log(&mut out, &data.events);
    render_interfaces(&mut out, &data.interfaces);
    render_product_info(&mut out, &data.product_info);
    render_spectrum(&mut out, &data.spectrum);
    render_cpe(&mut out, data.cpe_static, data.cpe_dynamic);
    render_service_flow_configs(&mut out, &data.service_flow_configs);

    out
}

fn render_downstream_qam(out: &mut String, channels: &[crate::parser::DownstreamQam]) {
    if channels.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_downstream_qam_power_dbmv",
        "Downstream SC-QAM power level in dBmV",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_downstream_qam_power_dbmv",
            &[
                ("channel", &ch.channel.to_string()),
                ("dcid", &ch.dcid.to_string()),
                ("frequency_mhz", &format!("{:.2}", ch.freq_mhz)),
                ("modulation", &ch.modulation),
            ],
            ch.power_dbmv,
        );
    }

    header(
        out,
        "cm3500_downstream_qam_snr_db",
        "Downstream SC-QAM signal-to-noise ratio in dB",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_downstream_qam_snr_db",
            &[
                ("channel", &ch.channel.to_string()),
                ("dcid", &ch.dcid.to_string()),
                ("frequency_mhz", &format!("{:.2}", ch.freq_mhz)),
                ("modulation", &ch.modulation),
            ],
            ch.snr_db,
        );
    }

    header(
        out,
        "cm3500_downstream_qam_octets_total",
        "Total octets received on downstream SC-QAM channel",
        "counter",
    );
    for ch in channels {
        counter(
            out,
            "cm3500_downstream_qam_octets_total",
            &[
                ("channel", &ch.channel.to_string()),
                ("dcid", &ch.dcid.to_string()),
            ],
            ch.octets,
        );
    }

    header(
        out,
        "cm3500_downstream_qam_corrected_total",
        "Total corrected errors on downstream SC-QAM channel",
        "counter",
    );
    for ch in channels {
        counter(
            out,
            "cm3500_downstream_qam_corrected_total",
            &[
                ("channel", &ch.channel.to_string()),
                ("dcid", &ch.dcid.to_string()),
            ],
            ch.correcteds,
        );
    }

    header(
        out,
        "cm3500_downstream_qam_uncorrectable_total",
        "Total uncorrectable errors on downstream SC-QAM channel",
        "counter",
    );
    for ch in channels {
        counter(
            out,
            "cm3500_downstream_qam_uncorrectable_total",
            &[
                ("channel", &ch.channel.to_string()),
                ("dcid", &ch.dcid.to_string()),
            ],
            ch.uncorrectables,
        );
    }
}

fn render_downstream_ofdm(out: &mut String, channels: &[crate::parser::DownstreamOfdm]) {
    if channels.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_downstream_ofdm_channel_width_mhz",
        "Downstream OFDM channel width in MHz",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_downstream_ofdm_channel_width_mhz",
            &[
                ("channel", &ch.channel.to_string()),
                ("fft_type", &ch.fft_type),
            ],
            ch.channel_width_mhz,
        );
    }

    header(
        out,
        "cm3500_downstream_ofdm_active_subcarriers",
        "Number of active subcarriers on downstream OFDM channel",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_downstream_ofdm_active_subcarriers",
            &[("channel", &ch.channel.to_string())],
            ch.active_subcarriers as f64,
        );
    }

    header(
        out,
        "cm3500_downstream_ofdm_rxmer_db",
        "Downstream OFDM receive modulation error ratio in dB",
        "gauge",
    );
    for ch in channels {
        for (typ, val) in [
            ("pilot", ch.rxmer_pilot_db),
            ("plc", ch.rxmer_plc_db),
            ("data", ch.rxmer_data_db),
        ] {
            gauge(
                out,
                "cm3500_downstream_ofdm_rxmer_db",
                &[("channel", &ch.channel.to_string()), ("measurement", typ)],
                val,
            );
        }
    }
}

fn render_upstream_qam(out: &mut String, channels: &[crate::parser::UpstreamQam]) {
    if channels.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_upstream_qam_power_dbmv",
        "Upstream SC-QAM transmit power in dBmV",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_upstream_qam_power_dbmv",
            &[
                ("channel", &ch.channel.to_string()),
                ("ucid", &ch.ucid.to_string()),
                ("frequency_mhz", &format!("{:.2}", ch.freq_mhz)),
                ("modulation", &ch.modulation),
                ("channel_type", &ch.channel_type),
            ],
            ch.power_dbmv,
        );
    }

    header(
        out,
        "cm3500_upstream_qam_symbol_rate_ksym",
        "Upstream SC-QAM symbol rate in kSym/s",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_upstream_qam_symbol_rate_ksym",
            &[
                ("channel", &ch.channel.to_string()),
                ("ucid", &ch.ucid.to_string()),
            ],
            ch.symbol_rate_ksym,
        );
    }
}

fn render_upstream_ofdm(out: &mut String, channels: &[crate::parser::UpstreamOfdm]) {
    if channels.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_upstream_ofdma_tx_power_dbmv",
        "Upstream OFDMA transmit power in dBmV",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_upstream_ofdma_tx_power_dbmv",
            &[
                ("channel", &ch.channel.to_string()),
                ("fft_type", &ch.fft_type),
            ],
            ch.tx_power_dbmv,
        );
    }

    header(
        out,
        "cm3500_upstream_ofdma_channel_width_mhz",
        "Upstream OFDMA channel width in MHz",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_upstream_ofdma_channel_width_mhz",
            &[("channel", &ch.channel.to_string())],
            ch.channel_width_mhz,
        );
    }

    header(
        out,
        "cm3500_upstream_ofdma_active_subcarriers",
        "Number of active subcarriers on upstream OFDMA channel",
        "gauge",
    );
    for ch in channels {
        gauge(
            out,
            "cm3500_upstream_ofdma_active_subcarriers",
            &[("channel", &ch.channel.to_string())],
            ch.active_subcarriers as f64,
        );
    }

    header(
        out,
        "cm3500_upstream_ofdma_active_subcarrier_range_mhz",
        "First and last active upstream OFDMA subcarrier positions in MHz",
        "gauge",
    );
    for ch in channels {
        for (edge, value) in [
            ("first", ch.first_active_subcarrier_mhz),
            ("last", ch.last_active_subcarrier_mhz),
        ] {
            gauge(
                out,
                "cm3500_upstream_ofdma_active_subcarrier_range_mhz",
                &[("channel", &ch.channel.to_string()), ("edge", edge)],
                value,
            );
        }
    }
}

fn render_dhcp(out: &mut String, dhcp: &crate::parser::DhcpInfo, dhcp_state: &str) {
    if !dhcp_state.is_empty() {
        labeled_metric(
            out,
            "cm3500_dhcp_state",
            "DHCP state of the cable modem",
            "gauge",
            &[("state", dhcp_state)],
            "1",
        );
    }

    if let Some(v) = dhcp.lease_total_secs {
        metric(
            out,
            "cm3500_dhcp_lease_total_seconds",
            "Total DHCP lease duration in seconds",
            "gauge",
            &v.to_string(),
        );
    }
    if let Some(v) = dhcp.lease_remaining_secs {
        metric(
            out,
            "cm3500_dhcp_lease_remaining_seconds",
            "DHCP lease time remaining in seconds",
            "gauge",
            &v.to_string(),
        );
    }
    if let Some(v) = dhcp.rebind_remaining_secs {
        metric(
            out,
            "cm3500_dhcp_rebind_remaining_seconds",
            "DHCP rebind time remaining in seconds",
            "gauge",
            &v.to_string(),
        );
    }
    if let Some(v) = dhcp.renew_remaining_secs {
        metric(
            out,
            "cm3500_dhcp_renew_remaining_seconds",
            "DHCP renew time remaining in seconds",
            "gauge",
            &v.to_string(),
        );
    }
}

fn render_qos(out: &mut String, flows: &[crate::parser::QosFlow]) {
    if flows.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_qos_packets_total",
        "Total packets on QoS service flow",
        "counter",
    );
    for f in flows {
        counter(
            out,
            "cm3500_qos_packets_total",
            &[
                ("sfid", &f.sfid.to_string()),
                ("direction", &f.direction),
                ("primary", if f.primary { "true" } else { "false" }),
                ("service_class", &f.service_class),
            ],
            f.packets,
        );
    }
}

fn render_cm_state(out: &mut String, state: &crate::parser::CmState) {
    if !state.overall_state.is_empty() {
        labeled_metric(
            out,
            "cm3500_docsis_state",
            "Overall DOCSIS registration state",
            "gauge",
            &[("state", &state.overall_state)],
            "1",
        );
    }

    if !state.phases.is_empty() {
        header(
            out,
            "cm3500_docsis_phase",
            "DOCSIS registration phase status (1 = current status for this phase)",
            "gauge",
        );
        for p in &state.phases {
            let phase_normalized = p.phase.to_lowercase().replace(['-', ' ', '/'], "_");
            gauge(
                out,
                "cm3500_docsis_phase",
                &[("phase", &phase_normalized), ("status", &p.status)],
                1.0,
            );
        }
    }

    if !state.bpi_status.is_empty() {
        labeled_metric(
            out,
            "cm3500_bpi_state",
            "Baseline Privacy Interface state",
            "gauge",
            &[("status", &state.bpi_status)],
            "1",
        );
    }

    if !state.tod_status.is_empty() {
        labeled_metric(
            out,
            "cm3500_tod_state",
            "Time of Day acquisition state",
            "gauge",
            &[("status", &state.tod_status)],
            "1",
        );
    }
}

fn render_event_log(out: &mut String, events: &[crate::parser::EventLogEntry]) {
    if events.is_empty() {
        return;
    }

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for e in events {
        *counts.entry(&e.event_id).or_insert(0) += 1;
    }

    header(
        out,
        "cm3500_event_log_entries",
        "Number of entries in the modem event log by event ID",
        "gauge",
    );
    for (event_id, count) in &counts {
        gauge(
            out,
            "cm3500_event_log_entries",
            &[("event_id", event_id)],
            *count as f64,
        );
    }

    let mut t3_timeouts = 0u32;
    let mut t4_timeouts = 0u32;
    let mut dhcp_failures = 0u32;
    let mut dhcp_renew_warnings = 0u32;
    let mut dhcp_renew_no_response = 0u32;
    let mut profile_changes = 0u32;
    let mut us_profiles: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut ofdma_profiles: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    for e in events {
        match e.event_id.as_str() {
            "82000200" => t3_timeouts += 1,
            "82000300" => t4_timeouts += 1,
            "68001202" => dhcp_failures += 1,
            "68010300" => dhcp_renew_warnings += 1,
            "68010100" => dhcp_renew_no_response += 1,
            "67061601" => profile_changes += 1,
            _ => {}
        }

        if let Some((chan, profile)) = extract_us_profile(&e.description) {
            us_profiles.insert(chan, profile);
        }
        if let Some((chan, profile)) = extract_ofdma_profile(&e.description) {
            ofdma_profiles.insert(chan, profile);
        }
    }

    metric(
        out,
        "cm3500_event_t3_timeouts",
        "T3 timeouts (No Ranging Response) in event log",
        "gauge",
        &t3_timeouts.to_string(),
    );
    metric(
        out,
        "cm3500_event_t4_timeouts",
        "T4 timeouts in event log",
        "gauge",
        &t4_timeouts.to_string(),
    );
    metric(
        out,
        "cm3500_event_dhcp_failures",
        "DHCP failures in event log",
        "gauge",
        &dhcp_failures.to_string(),
    );
    metric(
        out,
        "cm3500_event_dhcp_renew_warnings",
        "DHCP renew warnings in event log",
        "gauge",
        &dhcp_renew_warnings.to_string(),
    );
    metric(
        out,
        "cm3500_event_dhcp_renew_no_response",
        "DHCP renew sent with no response events in event log",
        "gauge",
        &dhcp_renew_no_response.to_string(),
    );
    metric(
        out,
        "cm3500_event_profile_changes",
        "Upstream profile assignment changes in event log",
        "gauge",
        &profile_changes.to_string(),
    );

    if !us_profiles.is_empty() {
        header(
            out,
            "cm3500_event_upstream_active_profile",
            "Last seen upstream profile ID from modem event log",
            "gauge",
        );
        let mut items = us_profiles.into_iter().collect::<Vec<_>>();
        items.sort();
        for (channel, profile) in items {
            gauge(
                out,
                "cm3500_event_upstream_active_profile",
                &[("channel", &channel)],
                profile as f64,
            );
        }
    }

    if !ofdma_profiles.is_empty() {
        header(
            out,
            "cm3500_event_ofdma_profile_id",
            "Last seen OFDM/OFDMA profile ID from modem event log",
            "gauge",
        );
        let mut items = ofdma_profiles.into_iter().collect::<Vec<_>>();
        items.sort();
        for (channel, profile) in items {
            gauge(
                out,
                "cm3500_event_ofdma_profile_id",
                &[("channel", &channel)],
                profile as f64,
            );
        }
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

fn render_product_info(out: &mut String, product: &crate::parser::ProductInfo) {
    if !product.ethernet_phy_type.is_empty() {
        labeled_metric(
            out,
            "cm3500_product_ethernet_phy_info",
            "Ethernet PHY type reported by the modem",
            "gauge",
            &[("phy_type", &product.ethernet_phy_type)],
            "1",
        );
    }

    if !product.logging_components_enabled.is_empty() {
        header(
            out,
            "cm3500_product_logging_components_enabled",
            "Number of enabled internal logging components by product debug group",
            "gauge",
        );
        for (group, count) in &product.logging_components_enabled {
            gauge(
                out,
                "cm3500_product_logging_components_enabled",
                &[("group", group)],
                *count as f64,
            );
        }
    }
}

fn render_spectrum(out: &mut String, spectrum: &[crate::parser::SpectrumChunk]) {
    if spectrum.is_empty() {
        return;
    }

    metric(
        out,
        "cm3500_spectrum_chunks",
        "Number of spectrum scan chunks returned by the modem",
        "gauge",
        &spectrum.len().to_string(),
    );

    header(
        out,
        "cm3500_spectrum_chunk_power_dbmv",
        "Spectrum scan chunk power summary in dBmV",
        "gauge",
    );
    header(
        out,
        "cm3500_spectrum_chunk_metadata",
        "Spectrum scan chunk metadata",
        "gauge",
    );
    header(
        out,
        "cm3500_spectrum_bin_power_dbmv",
        "Spectrum scan bin power in dBmV",
        "gauge",
    );

    for chunk in spectrum {
        let Some((&min_raw, &max_raw)) = chunk
            .bins_raw_tenth_dbmv
            .iter()
            .min()
            .zip(chunk.bins_raw_tenth_dbmv.iter().max())
        else {
            continue;
        };
        let avg_raw = chunk
            .bins_raw_tenth_dbmv
            .iter()
            .map(|v| *v as f64)
            .sum::<f64>()
            / chunk.bins_raw_tenth_dbmv.len() as f64;
        let center_mhz = format!("{:.3}", chunk.center_freq_hz as f64 / 1_000_000.0);

        for (stat, value) in [
            ("min", min_raw as f64 / 10.0),
            ("avg", avg_raw / 10.0),
            ("max", max_raw as f64 / 10.0),
        ] {
            gauge(
                out,
                "cm3500_spectrum_chunk_power_dbmv",
                &[
                    ("chunk", &chunk.chunk.to_string()),
                    ("center_mhz", &center_mhz),
                    ("stat", stat),
                ],
                value,
            );
        }

        for (kind, value) in [
            ("span_hz", chunk.span_hz as f64),
            ("bin_spacing_hz", chunk.bin_spacing_hz as f64),
            ("resolution_bandwidth", chunk.resolution_bandwidth as f64),
        ] {
            gauge(
                out,
                "cm3500_spectrum_chunk_metadata",
                &[
                    ("chunk", &chunk.chunk.to_string()),
                    ("center_mhz", &center_mhz),
                    ("kind", kind),
                ],
                value,
            );
        }

        let start_hz = chunk.center_freq_hz - (chunk.span_hz / 2);
        for (idx, raw) in chunk.bins_raw_tenth_dbmv.iter().enumerate() {
            let freq_hz = start_hz + (idx as i64 * chunk.bin_spacing_hz);
            gauge(
                out,
                "cm3500_spectrum_bin_power_dbmv",
                &[
                    ("chunk", &chunk.chunk.to_string()),
                    ("bin", &idx.to_string()),
                    (
                        "frequency_mhz",
                        &format!("{:.6}", freq_hz as f64 / 1_000_000.0),
                    ),
                ],
                *raw as f64 / 10.0,
            );
        }
    }
}

fn render_interfaces(out: &mut String, interfaces: &[crate::parser::InterfaceInfo]) {
    if interfaces.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_interface_up",
        "Interface operational status (1 = Up)",
        "gauge",
    );
    for iface in interfaces {
        let up = if iface.state == "Up" { 1.0 } else { 0.0 };
        gauge(
            out,
            "cm3500_interface_up",
            &[
                ("name", &iface.name),
                ("provisioned", &iface.provisioned),
                ("mac_address", &iface.mac_address),
            ],
            up,
        );
    }
}

fn render_cpe(out: &mut String, cpe_static: u32, cpe_dynamic: u32) {
    metric(
        out,
        "cm3500_cpe_static",
        "Number of static CPE devices detected",
        "gauge",
        &cpe_static.to_string(),
    );
    metric(
        out,
        "cm3500_cpe_dynamic",
        "Number of dynamic CPE devices detected",
        "gauge",
        &cpe_dynamic.to_string(),
    );
}

fn render_service_flow_configs(out: &mut String, flows: &[crate::parser::ServiceFlowConfig]) {
    if flows.is_empty() {
        return;
    }

    header(
        out,
        "cm3500_qos_max_traffic_rate_bps",
        "Maximum traffic rate for the QoS service flow from config (bps)",
        "gauge",
    );
    for f in flows {
        gauge(
            out,
            "cm3500_qos_max_traffic_rate_bps",
            &[
                ("direction", &f.direction),
                ("flow_index", &f.index.to_string()),
            ],
            f.max_traffic_rate_bps as f64,
        );
    }

    header(
        out,
        "cm3500_qos_min_reserved_rate_bps",
        "Minimum reserved rate for the QoS service flow from config (bps)",
        "gauge",
    );
    for f in flows {
        gauge(
            out,
            "cm3500_qos_min_reserved_rate_bps",
            &[
                ("direction", &f.direction),
                ("flow_index", &f.index.to_string()),
            ],
            f.min_reserved_rate_bps as f64,
        );
    }

    header(
        out,
        "cm3500_qos_max_traffic_burst",
        "Maximum traffic burst for the QoS service flow from config",
        "gauge",
    );
    for f in flows {
        gauge(
            out,
            "cm3500_qos_max_traffic_burst",
            &[
                ("direction", &f.direction),
                ("flow_index", &f.index.to_string()),
            ],
            f.max_traffic_burst as f64,
        );
    }

    header(
        out,
        "cm3500_qos_traffic_priority",
        "Traffic priority for the QoS service flow from config",
        "gauge",
    );
    for f in flows {
        gauge(
            out,
            "cm3500_qos_traffic_priority",
            &[
                ("direction", &f.direction),
                ("flow_index", &f.index.to_string()),
            ],
            f.traffic_priority as f64,
        );
    }
}

// Formatting helpers

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn format_labels(labels: &[(&str, &str)]) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = labels
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, escape(v)))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn header(out: &mut String, name: &str, help: &str, typ: &str) {
    out.push_str(&format!("# HELP {} {}\n", name, help));
    out.push_str(&format!("# TYPE {} {}\n", name, typ));
}

fn gauge(out: &mut String, name: &str, labels: &[(&str, &str)], value: f64) {
    out.push_str(&format!("{}{} {}\n", name, format_labels(labels), value));
}

fn counter(out: &mut String, name: &str, labels: &[(&str, &str)], value: u64) {
    out.push_str(&format!("{}{} {}\n", name, format_labels(labels), value));
}

fn metric(out: &mut String, name: &str, help: &str, typ: &str, value: &str) {
    header(out, name, help, typ);
    out.push_str(&format!("{} {}\n\n", name, value));
}

fn metric_f(out: &mut String, name: &str, help: &str, typ: &str, value: f64) {
    header(out, name, help, typ);
    out.push_str(&format!("{} {}\n\n", name, value));
}

fn labeled_metric(
    out: &mut String,
    name: &str,
    help: &str,
    typ: &str,
    labels: &[(&str, &str)],
    value: &str,
) {
    header(out, name, help, typ);
    out.push_str(&format!("{}{} {}\n\n", name, format_labels(labels), value));
}

pub fn render_error_metrics(err: &str, scrape_duration_secs: f64) -> String {
    let mut out = String::new();
    metric(
        &mut out,
        "cm3500_up",
        "Whether the modem scrape was successful",
        "gauge",
        "0",
    );
    metric_f(
        &mut out,
        "cm3500_scrape_duration_seconds",
        "Duration of the last scrape in seconds",
        "gauge",
        scrape_duration_secs,
    );
    labeled_metric(
        &mut out,
        "cm3500_scrape_error",
        "Last scrape error message",
        "gauge",
        &[("error", err)],
        "1",
    );
    out
}
