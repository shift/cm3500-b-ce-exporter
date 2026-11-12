use crate::parser::{ParseStatus, ScrapedData};

pub fn render_metrics(data: &ScrapedData, capacity_margin_percent: u8) -> String {
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
    metric(
        &mut out,
        "cm3500_parse_success",
        "Whether the required modem page fields parsed successfully",
        "gauge",
        "1",
    );
    render_section_parse_status(&mut out, &data.parse_status);

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
            (
                "firmware_build_time",
                &data.version_info.firmware_build_time,
            ),
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

    render_status_info(&mut out, &data.status_info, &data.version_info);
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
    render_diplexer(&mut out, &data.diplexer_bands);
    render_spectrum(&mut out, &data.spectrum);
    if let Some(age) = data.spectrum_age_seconds {
        metric(
            &mut out,
            "cm3500_spectrum_scan_age_seconds",
            "Age of the cached spectrum scan in seconds",
            "gauge",
            &age.to_string(),
        );
    }
    render_cpe(&mut out, data.cpe_static, data.cpe_dynamic);
    render_service_flow_configs(&mut out, &data.service_flow_configs);
    render_packet_classifications(&mut out, &data.packet_classifications);
    render_common_config(&mut out, &data.common_config);
    render_capacity(
        &mut out,
        &data.service_flow_configs,
        capacity_margin_percent,
    );
    render_dhcp_log(&mut out, &data.dhcp_log);
    render_dsx_log(&mut out, &data.dsx_log);

    dedupe_metric_headers(out)
}

fn dedupe_metric_headers(out: String) -> String {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut result = String::with_capacity(out.len());
    for line in out.lines() {
        let key = line
            .strip_prefix("# HELP ")
            .map(|rest| ("help", rest.split_whitespace().next().unwrap_or("")))
            .or_else(|| {
                line.strip_prefix("# TYPE ")
                    .map(|rest| ("type", rest.split_whitespace().next().unwrap_or("")))
            });
        if let Some(key) = key {
            if !seen.insert((key.0, key.1.to_string())) {
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

fn render_capacity(
    out: &mut String,
    flows: &[crate::parser::ServiceFlowConfig],
    margin_percent: u8,
) {
    header(
        out,
        "cm3500_capacity_provisioned_bps",
        "Highest individual provisioned service-flow rate per direction (not aggregate or measured throughput)",
        "gauge",
    );
    header(
        out,
        "cm3500_capacity_estimated_bps",
        "Estimated usable capacity after applying the configured margin (not measured throughput)",
        "gauge",
    );
    for (direction, label) in [("Upstream", "upstream"), ("Downstream", "downstream")] {
        let max_bps = flows
            .iter()
            .filter(|f| f.direction.eq_ignore_ascii_case(direction))
            .map(|f| f.max_traffic_rate_bps)
            .max();
        let Some(provisioned_bps) = max_bps else {
            continue;
        };
        let estimated_bps = provisioned_bps.saturating_mul(margin_percent as u64) / 100;
        let margin_label = margin_percent.to_string();
        gauge(
            out,
            "cm3500_capacity_provisioned_bps",
            &[
                ("direction", label),
                ("basis", "max_individual_service_flow_rate"),
            ],
            provisioned_bps as f64,
        );
        gauge(
            out,
            "cm3500_capacity_estimated_bps",
            &[
                ("direction", label),
                ("basis", "max_individual_service_flow_rate"),
                ("margin_percent", margin_label.as_str()),
            ],
            estimated_bps as f64,
        );
    }
    out.push('\n');
}

fn render_status_info(
    out: &mut String,
    info: &crate::parser::StatusInfo,
    version: &crate::parser::VersionInfo,
) {
    labeled_metric(
        out,
        "cm3500_status_info",
        "Static modem status and product information",
        "gauge",
        &[
            ("product_type", &info.product_type),
            ("hardware_info", &info.hardware_info),
            ("system", &version.system),
            ("router", bool_label(info.router_enabled)),
            ("telemetry", bool_label(info.telemetry_enabled)),
            ("rf_hw", &info.rf_hw),
            ("market", &info.market),
        ],
        "1",
    );

    if !info.l2vpn_attachment_circuit_ids.is_empty() {
        header(
            out,
            "cm3500_l2vpn_attachment_circuit_info",
            "L2VPN attachment circuit identifier",
            "gauge",
        );
        for (index, id) in info.l2vpn_attachment_circuit_ids.iter().enumerate() {
            gauge(
                out,
                "cm3500_l2vpn_attachment_circuit_info",
                &[("index", &(index + 1).to_string()), ("id", id)],
                1.0,
            );
        }
        out.push('\n');
    }
}

fn bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "unknown",
    }
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
        "cm3500_downstream_ofdm_active_subcarrier_range_mhz",
        "First and last active downstream OFDM subcarrier in MHz",
        "gauge",
    );
    for ch in channels {
        for (edge, value) in [
            ("first", ch.first_active_subcarrier_mhz),
            ("last", ch.last_active_subcarrier_mhz),
        ] {
            gauge(
                out,
                "cm3500_downstream_ofdm_active_subcarrier_range_mhz",
                &[("channel", &ch.channel.to_string()), ("edge", edge)],
                value,
            );
        }
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

    if channels.iter().any(|ch| !ch.unlabeled_values.is_empty()) {
        header(
            out,
            "cm3500_upstream_ofdma_unlabeled_value",
            "Upstream OFDMA values present in the modem row but not named by its table header",
            "gauge",
        );
        for ch in channels {
            for (index, value) in ch.unlabeled_values.iter().enumerate() {
                gauge(
                    out,
                    "cm3500_upstream_ofdma_unlabeled_value",
                    &[
                        ("channel", &ch.channel.to_string()),
                        ("column_index", &(index + 7).to_string()),
                    ],
                    *value,
                );
            }
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
    if let Some(v) = dhcp.rebind_total_secs {
        metric(
            out,
            "cm3500_dhcp_rebind_total_seconds",
            "Total DHCP rebind duration in seconds",
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
    if let Some(v) = dhcp.renew_total_secs {
        metric(
            out,
            "cm3500_dhcp_renew_total_seconds",
            "Total DHCP renew duration in seconds",
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

    for (name, help, value) in [
        (
            "cm3500_dhcp_cm_ip_info",
            "Cable modem IP address assigned by DHCP",
            &dhcp.cm_ip,
        ),
        (
            "cm3500_dhcp_cm_subnet_info",
            "Cable modem DHCP subnet mask",
            &dhcp.cm_subnet,
        ),
        (
            "cm3500_dhcp_cm_gateway_info",
            "Cable modem DHCP gateway",
            &dhcp.cm_gateway,
        ),
        (
            "cm3500_dhcp_tftp_server_info",
            "DHCP-provided TFTP server",
            &dhcp.tftp_server,
        ),
        (
            "cm3500_dhcp_time_server_info",
            "DHCP-provided time server",
            &dhcp.time_server,
        ),
        (
            "cm3500_dhcp_boot_file_info",
            "DHCP-provided cable modem boot file",
            &dhcp.boot_file,
        ),
        (
            "cm3500_dhcp_software_filename_info",
            "Software filename reported by the modem",
            &dhcp.software_filename,
        ),
    ] {
        if !value.is_empty() {
            labeled_metric(out, name, help, "gauge", &[("value", value)], "1");
        }
    }

    if let Some(v) = dhcp.time_offset_seconds {
        metric(
            out,
            "cm3500_dhcp_time_offset_seconds",
            "DHCP time offset in seconds",
            "gauge",
            &v.to_string(),
        );
    }
    if let Some(v) = dhcp.mdd_ip_override {
        metric(
            out,
            "cm3500_dhcp_mdd_ip_override",
            "MAC Domain Descriptor IP override value",
            "gauge",
            &v.to_string(),
        );
    }
    if let Some(v) = dhcp.mdd_ip_override_enabled {
        metric(
            out,
            "cm3500_dhcp_mdd_ip_override_enabled",
            "Whether the MDD IP override is enabled",
            "gauge",
            if v { "1" } else { "0" },
        );
    }
    if let Some(v) = dhcp.mdd_ip_mode {
        labeled_metric(
            out,
            "cm3500_dhcp_mdd_ip_mode",
            "MAC Domain Descriptor IP mode",
            "gauge",
            &[("mode", &dhcp.mdd_ip_mode_name)],
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

    if state.dhcp_ipv4_attempts.is_some() || state.dhcp_ipv6_attempts.is_some() {
        header(
            out,
            "cm3500_dhcp_attempts",
            "Number of DHCP attempts to obtain a CM IP address",
            "gauge",
        );
        for (family, attempts) in [
            ("ipv4", state.dhcp_ipv4_attempts),
            ("ipv6", state.dhcp_ipv6_attempts),
        ] {
            if let Some(value) = attempts {
                gauge(
                    out,
                    "cm3500_dhcp_attempts",
                    &[("family", family)],
                    value as f64,
                );
            }
        }
        out.push('\n');
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

    if !product.logging_components.is_empty() {
        header(
            out,
            "cm3500_product_logging_component",
            "Per-component internal logging state (1 = enabled)",
            "gauge",
        );
        for component in &product.logging_components {
            gauge(
                out,
                "cm3500_product_logging_component",
                &[
                    ("index", &component.index.to_string()),
                    ("component", &component.name),
                ],
                if component.enabled { 1.0 } else { 0.0 },
            );
        }
    }

    if !product.severity_levels.is_empty() {
        header(
            out,
            "cm3500_product_severity_level",
            "Per-severity logging state (1 = enabled)",
            "gauge",
        );
        for level in &product.severity_levels {
            gauge(
                out,
                "cm3500_product_severity_level",
                &[
                    ("index", &level.index.to_string()),
                    ("severity", &level.name),
                ],
                if level.enabled { 1.0 } else { 0.0 },
            );
        }
    }
}

fn render_diplexer(out: &mut String, bands: &[crate::parser::DiplexerBand]) {
    if bands.is_empty() {
        return;
    }
    header(
        out,
        "cm3500_diplexer_band_info",
        "Diplexer band ranges and current band setting",
        "gauge",
    );
    for band in bands {
        gauge(
            out,
            "cm3500_diplexer_band_info",
            &[
                ("band", &band.band.to_string()),
                ("upstream_range", &band.upstream_range),
                ("downstream_range", &band.downstream_range),
                ("current", if band.current { "true" } else { "false" }),
            ],
            1.0,
        );
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
                ("speed_mbps", &iface.speed_mbps),
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
        "Maximum traffic rate for the QoS service flow from config (bits/s)",
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
        "Minimum reserved rate for the QoS service flow from config (bits/s)",
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

    header(
        out,
        "cm3500_qos_service_flow_info",
        "Service-flow identity and class information from config",
        "gauge",
    );
    for f in flows {
        gauge(
            out,
            "cm3500_qos_service_flow_info",
            &[
                ("direction", &f.direction),
                ("flow_index", &f.index.to_string()),
                ("reference", &f.reference.to_string()),
                ("sf_id", &f.sf_id.map(|v| v.to_string()).unwrap_or_default()),
                ("service_class", &f.service_class),
            ],
            1.0,
        );
    }

    for (name, help, values) in [
        (
            "cm3500_qos_set_type",
            "QoS parameter set type",
            flows
                .iter()
                .map(|f| f.qos_set_type as f64)
                .collect::<Vec<f64>>(),
        ),
        (
            "cm3500_qos_min_reserved_rate_pktsize",
            "Minimum reserved rate packet size",
            flows
                .iter()
                .map(|f| f.min_reserved_rate_pktsize as f64)
                .collect::<Vec<f64>>(),
        ),
        (
            "cm3500_qos_active_qos_timeout",
            "Active QoS timeout",
            flows
                .iter()
                .map(|f| f.active_qos_timeout as f64)
                .collect::<Vec<f64>>(),
        ),
        (
            "cm3500_qos_admitted_qos_timeout",
            "Admitted QoS timeout",
            flows
                .iter()
                .map(|f| f.admitted_qos_timeout as f64)
                .collect::<Vec<f64>>(),
        ),
        (
            "cm3500_qos_max_concat_burst",
            "Maximum concatenated burst",
            flows
                .iter()
                .map(|f| f.max_concat_burst as f64)
                .collect::<Vec<f64>>(),
        ),
    ] {
        header(out, name, help, "gauge");
        for (flow, value) in flows.iter().zip(values) {
            gauge(
                out,
                name,
                &[
                    ("direction", &flow.direction),
                    ("flow_index", &flow.index.to_string()),
                ],
                value,
            );
        }
    }
}

fn render_packet_classifications(
    out: &mut String,
    classifications: &[crate::parser::PacketClassification],
) {
    if classifications.is_empty() {
        return;
    }
    header(
        out,
        "cm3500_qos_packet_classification_info",
        "Packet classification rules from the modem configuration",
        "gauge",
    );
    for pc in classifications {
        gauge(
            out,
            "cm3500_qos_packet_classification_info",
            &[
                ("direction", &pc.direction),
                ("index", &pc.index.to_string()),
                ("service_flow", &pc.service_flow.to_string()),
                ("identifier", &pc.identifier.to_string()),
                ("rule_priority", &pc.rule_priority.to_string()),
                ("activation_state", &pc.activation_state.to_string()),
                ("ip_protocol", &pc.ip_protocol.to_string()),
                (
                    "source_port_start",
                    &pc.source_port_start
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                ),
                (
                    "source_port_end",
                    &pc.source_port_end
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                ),
                (
                    "dest_port_start",
                    &pc.dest_port_start
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                ),
                (
                    "dest_port_end",
                    &pc.dest_port_end.map(|v| v.to_string()).unwrap_or_default(),
                ),
            ],
            1.0,
        );
    }
}

fn render_common_config(out: &mut String, config: &crate::parser::CommonConfig) {
    for (name, help, value) in [
        (
            "cm3500_config_file_info",
            "Configuration file URL used by the modem",
            &config.config_file,
        ),
        (
            "cm3500_config_upgrade_server_info",
            "Configured software upgrade server",
            &config.upgrade_server,
        ),
    ] {
        if !value.is_empty() {
            labeled_metric(out, name, help, "gauge", &[("value", value)], "1");
        }
    }
    if let Some(v) = config.network_access {
        metric(
            out,
            "cm3500_config_network_access",
            "Whether network access is enabled in the modem configuration",
            "gauge",
            if v { "1" } else { "0" },
        );
    }
    if let Some(v) = config.max_cpe_allowed {
        metric(
            out,
            "cm3500_config_max_cpe_allowed",
            "Maximum CPE devices allowed by the modem configuration",
            "gauge",
            &v.to_string(),
        );
    }
    if let Some(v) = config.privacy_enable {
        metric(
            out,
            "cm3500_config_privacy_enable",
            "Privacy enable setting from the modem configuration",
            "gauge",
            &v.to_string(),
        );
    }
    if !config.snmp_mib.is_empty() {
        header(
            out,
            "cm3500_config_snmp_mib_info",
            "SNMP MIB settings from the modem configuration",
            "gauge",
        );
        for (index, value) in config.snmp_mib.iter().enumerate() {
            gauge(
                out,
                "cm3500_config_snmp_mib_info",
                &[("index", &index.to_string()), ("value", value)],
                1.0,
            );
        }
    }
}

fn render_dhcp_log(out: &mut String, entries: &[crate::parser::DhcpLogEntry]) {
    if entries.is_empty() {
        return;
    }
    metric(
        out,
        "cm3500_dhcp_log_entries",
        "Number of DHCPv6 log entries exposed by the modem",
        "gauge",
        &entries.len().to_string(),
    );

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for entry in entries {
        *counts.entry(&entry.message_type).or_insert(0) += 1;
    }
    header(
        out,
        "cm3500_dhcp_log_message_type_entries",
        "DHCPv6 log entry count by message type",
        "gauge",
    );
    for (message_type, count) in counts {
        gauge(
            out,
            "cm3500_dhcp_log_message_type_entries",
            &[("message_type", message_type)],
            count as f64,
        );
    }

    header(
        out,
        "cm3500_dhcp_log_entry_info",
        "DHCPv6 log entry details",
        "gauge",
    );
    for entry in entries {
        let options = entry
            .options
            .iter()
            .map(|(id, name)| format!("{}:{}", id, name))
            .collect::<Vec<_>>()
            .join(",");
        gauge(
            out,
            "cm3500_dhcp_log_entry_info",
            &[
                ("index", &entry.index.to_string()),
                ("timestamp", &entry.timestamp),
                ("interface", &entry.interface),
                ("message_type", &entry.message_type),
                ("client_id", &entry.client_id),
                ("server_id", &entry.server_id),
                (
                    "preference",
                    &entry.preference.map(|v| v.to_string()).unwrap_or_default(),
                ),
                (
                    "ia_type",
                    &entry.ia_type.map(|v| v.to_string()).unwrap_or_default(),
                ),
                (
                    "ia_id",
                    &entry.ia_id.map(|v| v.to_string()).unwrap_or_default(),
                ),
                ("t1", &entry.t1.map(|v| v.to_string()).unwrap_or_default()),
                ("t2", &entry.t2.map(|v| v.to_string()).unwrap_or_default()),
                (
                    "ia_count",
                    &entry.ia_count.map(|v| v.to_string()).unwrap_or_default(),
                ),
                (
                    "ia_addr_count",
                    &entry
                        .ia_addr_count
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                ),
                (
                    "options_len",
                    &entry.options_len.map(|v| v.to_string()).unwrap_or_default(),
                ),
                ("stateful_addr", &entry.stateful_addr),
                (
                    "preferred_lifetime_seconds",
                    &entry
                        .preferred_lifetime
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                ),
                (
                    "valid_lifetime_seconds",
                    &entry
                        .valid_lifetime
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                ),
                ("options", &options),
            ],
            1.0,
        );
    }
}

fn render_dsx_log(out: &mut String, entries: &[crate::parser::DsxLogEntry]) {
    metric(
        out,
        "cm3500_dsx_log_entries",
        "Number of DSX message log entries exposed by the modem",
        "gauge",
        &entries.len().to_string(),
    );
    if entries.is_empty() {
        return;
    }
    header(
        out,
        "cm3500_dsx_log_entry_info",
        "DSX message log entry details",
        "gauge",
    );
    for (index, entry) in entries.iter().enumerate() {
        gauge(
            out,
            "cm3500_dsx_log_entry_info",
            &[
                ("index", &index.to_string()),
                ("message", &entry.cells.join(" | ")),
            ],
            1.0,
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

fn render_section_parse_status(out: &mut String, status: &ParseStatus) {
    header(
        out,
        "cm3500_section_parse_success",
        "Whether each modem response section parsed successfully",
        "gauge",
    );
    for (section, ok) in status.iter() {
        out.push_str(&format!(
            "cm3500_section_parse_success{{section=\"{}\"}} {}\n",
            section,
            u8::from(ok)
        ));
    }
    out.push('\n');
}

pub fn render_error_metrics(_err: &str, scrape_duration_secs: f64) -> String {
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
    metric(
        &mut out,
        "cm3500_parse_success",
        "Whether the required modem page fields parsed successfully",
        "gauge",
        "0",
    );
    metric(
        &mut out,
        "cm3500_scrape_error",
        "Whether the last scrape encountered an error",
        "gauge",
        "1",
    );
    dedupe_metric_headers(out)
}

pub fn render_parse_error_metrics(
    err: &str,
    scrape_duration_secs: f64,
    status: &ParseStatus,
) -> String {
    let mut out = render_error_metrics(err, scrape_duration_secs);
    render_section_parse_status(&mut out, status);
    out
}
