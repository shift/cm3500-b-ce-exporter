use anyhow::Result;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct DownstreamQam {
    pub channel: u32,
    pub dcid: u32,
    pub freq_mhz: f64,
    pub power_dbmv: f64,
    pub snr_db: f64,
    pub modulation: String,
    pub octets: u64,
    pub correcteds: u64,
    pub uncorrectables: u64,
}

#[derive(Debug, Clone)]
pub struct DownstreamOfdm {
    pub channel: u32,
    pub fft_type: String,
    pub channel_width_mhz: f64,
    pub active_subcarriers: u32,
    pub rxmer_pilot_db: f64,
    pub rxmer_plc_db: f64,
    pub rxmer_data_db: f64,
}

#[derive(Debug, Clone)]
pub struct UpstreamQam {
    pub channel: u32,
    pub ucid: u32,
    pub freq_mhz: f64,
    pub power_dbmv: f64,
    pub channel_type: String,
    pub symbol_rate_ksym: f64,
    pub modulation: String,
}

#[derive(Debug, Clone)]
pub struct UpstreamOfdm {
    pub channel: u32,
    pub fft_type: String,
    pub channel_width_mhz: f64,
    pub active_subcarriers: u32,
    pub tx_power_dbmv: f64,
}

#[derive(Debug, Clone)]
pub struct QosFlow {
    pub sfid: u64,
    pub service_class: String,
    pub direction: String,
    pub primary: bool,
    pub packets: u64,
}

#[derive(Debug, Clone)]
pub struct DocsisPhase {
    pub phase: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub hardware_rev: String,
    pub vendor: String,
    pub bootloader: String,
    pub software_rev: String,
    pub model: String,
    pub serial: String,
    pub firmware_name: String,
    #[allow(dead_code)]
    pub firmware_build_time: String,
}

#[derive(Debug, Clone)]
pub struct DhcpInfo {
    #[allow(dead_code)]
    pub cm_ip: String,
    #[allow(dead_code)]
    pub cm_subnet: String,
    #[allow(dead_code)]
    pub cm_gateway: String,
    pub lease_total_secs: Option<u64>,
    pub lease_remaining_secs: Option<u64>,
    #[allow(dead_code)]
    pub rebind_total_secs: Option<u64>,
    pub rebind_remaining_secs: Option<u64>,
    #[allow(dead_code)]
    pub renew_total_secs: Option<u64>,
    pub renew_remaining_secs: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CmState {
    pub overall_state: String,
    pub phases: Vec<DocsisPhase>,
    pub bpi_status: String,
    pub tod_status: String,
}

#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub event_id: String,
    #[allow(dead_code)]
    pub event_level: u32,
    #[allow(dead_code)]
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub name: String,
    pub provisioned: String,
    pub state: String,
    pub mac_address: String,
}

#[derive(Debug, Clone)]
pub struct ServiceFlowConfig {
    pub direction: String,
    pub index: u32,
    pub max_traffic_rate_kbps: u64,
    pub max_traffic_burst: u64,
    pub min_reserved_rate_kbps: u64,
    pub traffic_priority: u32,
    pub scheduling_type: u32,
}

#[derive(Debug, Clone)]
pub struct ScrapedData {
    pub downstream_qam: Vec<DownstreamQam>,
    pub downstream_ofdm: Vec<DownstreamOfdm>,
    pub upstream_qam: Vec<UpstreamQam>,
    pub upstream_ofdm: Vec<UpstreamOfdm>,
    pub uptime_seconds: Option<u64>,
    pub cm_status: String,
    pub version_info: VersionInfo,
    pub dhcp_info: DhcpInfo,
    pub qos_flows: Vec<QosFlow>,
    pub cm_state: CmState,
    pub events: Vec<EventLogEntry>,
    pub interfaces: Vec<InterfaceInfo>,
    pub cpe_static: u32,
    pub cpe_dynamic: u32,
    pub dhcp_state: String,
    pub service_flow_configs: Vec<ServiceFlowConfig>,
    pub scrape_duration_secs: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn parse_all(
    status_html: &str,
    vers_html: &str,
    dhcp_html: &str,
    qos_html: &str,
    cm_state_html: &str,
    event_html: &str,
    config_params_html: &str,
    scrape_duration_secs: f64,
) -> Result<ScrapedData> {
    let (cpe_static, cpe_dynamic) = parse_cpe_counts(status_html);
    Ok(ScrapedData {
        downstream_qam: parse_downstream_qam(status_html),
        downstream_ofdm: parse_downstream_ofdm(status_html),
        upstream_qam: parse_upstream_qam(status_html),
        upstream_ofdm: parse_upstream_ofdm(status_html),
        uptime_seconds: parse_uptime(status_html),
        cm_status: parse_cm_status(status_html),
        version_info: parse_version_info(vers_html, status_html),
        dhcp_info: parse_dhcp_info(dhcp_html),
        dhcp_state: parse_dhcp_state(dhcp_html),
        qos_flows: parse_qos(qos_html),
        cm_state: parse_cm_state(cm_state_html),
        events: parse_event_log(event_html),
        interfaces: parse_interfaces(status_html),
        cpe_static,
        cpe_dynamic,
        service_flow_configs: parse_service_flow_configs(config_params_html),
        scrape_duration_secs,
    })
}

fn parse_downstream_qam(html: &str) -> Vec<DownstreamQam> {
    let re = Regex::new(
        r"Downstream\s+(\d+)</td><td>(\d+)</td><td>([\d.]+)\s*MHz</td><td>(-?[\d.]+)\s*dBmV</td><td>([\d.]+)\s*dB</td><td>(\d+QAM)</td><td>(\d+)</td><td>(\d+)</td><td>(\d+)"
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(DownstreamQam {
                channel: c[1].parse().ok()?,
                dcid: c[2].parse().ok()?,
                freq_mhz: c[3].parse().ok()?,
                power_dbmv: c[4].parse().ok()?,
                snr_db: c[5].parse().ok()?,
                modulation: c[6].to_string(),
                octets: c[7].parse().ok()?,
                correcteds: c[8].parse().ok()?,
                uncorrectables: c[9].parse().ok()?,
            })
        })
        .collect()
}

fn parse_downstream_ofdm(html: &str) -> Vec<DownstreamOfdm> {
    let re = Regex::new(
        r"Downstream\s+(\d+)</td><td>(\d+K)</td><td>([\d.]+)</td><td>(\d+)</td><td>([\d.]+)</td><td>([\d.]+)</td><td>([\d.]+)</td><td>([\d.]+)</td><td>([\d.]+)"
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(DownstreamOfdm {
                channel: c[1].parse().ok()?,
                fft_type: c[2].to_string(),
                channel_width_mhz: c[3].parse().ok()?,
                active_subcarriers: c[4].parse().ok()?,
                rxmer_pilot_db: c[7].parse().ok()?,
                rxmer_plc_db: c[8].parse().ok()?,
                rxmer_data_db: c[9].parse().ok()?,
            })
        })
        .collect()
}

fn parse_upstream_qam(html: &str) -> Vec<UpstreamQam> {
    let re = Regex::new(
        r"Upstream\s+(\d+)</td><td>(\d+)</td><td>([\d.]+)\s*MHz</td><td>([\d.]+)\s*dBmV</td><td>([^<]+)</td><td>([\d.]+)\s*kSym/s</td><td>(\d+QAM)"
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(UpstreamQam {
                channel: c[1].parse().ok()?,
                ucid: c[2].parse().ok()?,
                freq_mhz: c[3].parse().ok()?,
                power_dbmv: c[4].parse().ok()?,
                channel_type: c[5].trim().to_string(),
                symbol_rate_ksym: c[6].parse().ok()?,
                modulation: c[7].to_string(),
            })
        })
        .collect()
}

fn parse_upstream_ofdm(html: &str) -> Vec<UpstreamOfdm> {
    let re = Regex::new(
        r"Upstream\s+(\d+)</td><td>(\d+K)</td><td>([\d.]+)</td><td>(\d+)</td><td>([\d.]+)</td><td>([\d.]+)</td><td>(-?[\d.]+)"
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(UpstreamOfdm {
                channel: c[1].parse().ok()?,
                fft_type: c[2].to_string(),
                channel_width_mhz: c[3].parse().ok()?,
                active_subcarriers: c[4].parse().ok()?,
                tx_power_dbmv: c[7].parse().ok()?,
            })
        })
        .collect()
}

fn parse_uptime(html: &str) -> Option<u64> {
    let re =
        Regex::new(r"System Uptime:\s*</td>\s*<td>(\d+)\s*d:\s*(\d+)\s*h:\s*(\d+)\s*m").unwrap();
    let c = re.captures(html)?;
    let days: u64 = c[1].parse().ok()?;
    let hours: u64 = c[2].parse().ok()?;
    let mins: u64 = c[3].parse().ok()?;
    Some(days * 86400 + hours * 3600 + mins * 60)
}

fn parse_cm_status(html: &str) -> String {
    let re = Regex::new(r"CM Status:</td><td>([^<]+)").unwrap();
    re.captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default()
}

fn parse_version_info(vers_html: &str, status_html: &str) -> VersionInfo {
    let hw_rev = Regex::new(r"HW_REV:\s*([^\n<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let vendor = Regex::new(r"VENDOR:\s*([^\n<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let bootloader = Regex::new(r"BOOTR:\s*([^\n<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let software_rev = Regex::new(r"SW_REV:\s*([^\n<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let model = Regex::new(r"MODEL:\s*([^\n<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let serial = Regex::new(r"Serial Number:</td>\s*<td>([^<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let firmware_name = Regex::new(r"Firmware Name:</td><td>([^<]+)")
        .unwrap()
        .captures(vers_html)
        .or_else(|| {
            Regex::new(r"Firmware Name:</td>\s*<td>([^<]+)")
                .unwrap()
                .captures(status_html)
        })
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let firmware_build_time = Regex::new(r"Firmware Build Time:</td><td>([^<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    VersionInfo {
        hardware_rev: hw_rev,
        vendor,
        bootloader,
        software_rev,
        model,
        serial,
        firmware_name,
        firmware_build_time,
    }
}

fn parse_dhcp_info(html: &str) -> DhcpInfo {
    let cm_ip = Regex::new(r"CM IP Addr\s*</td>\s*<td[^>]*>([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let cm_subnet = Regex::new(r"CM Subnet Mask\s*</td>\s*<td>([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let cm_gateway = Regex::new(r"CM IP Gateway\s*</td>\s*<td>([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let lease_total_secs = parse_dhcp_time(html, "Lease:");
    let lease_remaining_secs = parse_dhcp_remaining(html, "Lease:");
    let rebind_total_secs = parse_dhcp_time(html, "Rebind:");
    let rebind_remaining_secs = parse_dhcp_remaining(html, "Rebind:");
    let renew_total_secs = parse_dhcp_time(html, "Renew:");
    let renew_remaining_secs = parse_dhcp_remaining(html, "Renew:");

    DhcpInfo {
        cm_ip,
        cm_subnet,
        cm_gateway,
        lease_total_secs,
        lease_remaining_secs,
        rebind_total_secs,
        rebind_remaining_secs,
        renew_total_secs,
        renew_remaining_secs,
    }
}

fn parse_dhcp_time(html: &str, label: &str) -> Option<u64> {
    let pattern = format!(r"{}<\s*/td>\s*<td[^>]*>(\d+)\s*sec", regex::escape(label));
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .and_then(|c| c[1].parse().ok())
}

fn parse_dhcp_remaining(html: &str, label: &str) -> Option<u64> {
    // Match lines like: "Lease:</td><td width=310>600 sec (303 sec remaining)"
    // The pattern after the label: total sec (remaining sec remaining)
    let pattern = format!(
        r"{}<\s*/td>\s*<td[^>]*>\d+\s*sec\s*\((\d+)\s*sec\s*remaining\)",
        regex::escape(label)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .and_then(|c| c[1].parse().ok())
}

fn parse_qos(html: &str) -> Vec<QosFlow> {
    let re = Regex::new(
        r"<tr><td>(\d+)</td><td>([^<]*)</td><td>(Upstream|Downstream)</td><td>(Yes|No)</td><td>(\d+)</td></tr>"
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(QosFlow {
                sfid: c[1].parse().ok()?,
                service_class: c[2].trim().to_string(),
                direction: c[3].to_string(),
                primary: &c[4] == "Yes",
                packets: c[5].parse().ok()?,
            })
        })
        .collect()
}

fn parse_cm_state(html: &str) -> CmState {
    let overall_state = Regex::new(r"<b>CM State:</b>([^<]+)<br>")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let phase_re = Regex::new(r"<td[^>]*>(Docsis-[^<]+)</td>\s*<td[^>]*>([^<]+)</td>").unwrap();
    let phases = phase_re
        .captures_iter(html)
        .map(|c| DocsisPhase {
            phase: c[1].trim().to_string(),
            status: c[2].trim().to_string(),
        })
        .collect();

    let bpi_status = Regex::new(r"BPI Status\s*</td>\s*<td[^>]*>([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let tod_status = Regex::new(r"Time of Day\s*</td>\s*<td[^>]*>([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    CmState {
        overall_state,
        phases,
        bpi_status,
        tod_status,
    }
}

fn parse_event_log(html: &str) -> Vec<EventLogEntry> {
    let re = Regex::new(
        r#"<td\s+align=center>([^<]+)</td>\s*<td\s+align=center>(\d+)</td>\s*<td\s+align=center>(\d+)</td>\s*<td\s+align=left>([^<]+)</td>"#
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(EventLogEntry {
                event_id: c[2].to_string(),
                event_level: c[3].parse().ok()?,
                description: c[4].trim().to_string(),
            })
        })
        .collect()
}

fn parse_interfaces(html: &str) -> Vec<InterfaceInfo> {
    // Find the specific "Interface Parameters" section
    let section = Regex::new(r"(?s)Interface Parameters.*?</table>")
        .unwrap()
        .captures(html)
        .map(|c| c[0].to_string())
        .unwrap_or_default();

    let re = Regex::new(
        r"<tr><td>([^<]+)</td>\s*<td>([^<]+)</td>\s*<td>([^<]+)</td>\s*<td>([^<]*)</td>\s*<td>([^<]+)</td>"
    ).unwrap();

    re.captures_iter(&section)
        .filter_map(|c| {
            let name = c[1].trim().to_string();
            // Skip header rows
            if name == "Interface Name" {
                return None;
            }
            Some(InterfaceInfo {
                name,
                provisioned: c[2].trim().to_string(),
                state: c[3].trim().to_string(),
                mac_address: c[5].trim().to_string(),
            })
        })
        .collect()
}

fn parse_cpe_counts(html: &str) -> (u32, u32) {
    let re = Regex::new(r"staticCPE\((\d+)\),\s*dynamicCPE\((\d+)\)").unwrap();
    if let Some(c) = re.captures(html) {
        (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0))
    } else {
        (0, 0)
    }
}

fn parse_dhcp_state(html: &str) -> String {
    Regex::new(r"CM\s+Dhcp\s+State\s*:\s*</td>\s*<td>([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default()
}

fn parse_service_flow_configs(html: &str) -> Vec<ServiceFlowConfig> {
    let mut flows = Vec::new();

    // Split on "UpstreamServiceFlow" and "DownstreamServiceFlow"
    let dir_re = Regex::new(r"(Upstream|Downstream)ServiceFlow\s*=").unwrap();
    let max_rate_re = Regex::new(r"SfMaxTrafficRate\s*=\s*(\d+)").unwrap();
    let max_burst_re = Regex::new(r"SfMaxTrafficBurst\s*=\s*(\d+)").unwrap();
    let min_rate_re = Regex::new(r"SfMinReservedRate\s*=\s*(\d+)").unwrap();
    let priority_re = Regex::new(r"SfTrafficPriority\s*=\s*(\d+)").unwrap();
    let sched_re = Regex::new(r"SfSchedulingType\s*=\s*(\d+)").unwrap();

    for (i, cap) in dir_re.captures_iter(html).enumerate() {
        let direction = cap[1].to_string();
        // Find the position of this match and extract a chunk of text after it
        let start = cap.get(0).unwrap().end();
        // Grab ~500 chars or until the next ServiceFlow block
        let remaining = &html[start..];
        let end = dir_re
            .find(remaining)
            .map(|m| m.start())
            .unwrap_or(remaining.len().min(500));
        let chunk = &remaining[..end];

        let max_rate = max_rate_re
            .captures(chunk)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);
        let max_burst = max_burst_re
            .captures(chunk)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);
        let min_rate = min_rate_re
            .captures(chunk)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);
        let priority = priority_re
            .captures(chunk)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);
        let sched = sched_re
            .captures(chunk)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(0);

        flows.push(ServiceFlowConfig {
            direction,
            index: i as u32,
            max_traffic_rate_kbps: max_rate,
            max_traffic_burst: max_burst,
            min_reserved_rate_kbps: min_rate,
            traffic_priority: priority,
            scheduling_type: sched,
        });
    }

    flows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dhcp_remaining() {
        let html = r#"Lease:</td><td width=310>600 sec (303 sec remaining)</td></tr>"#;
        assert_eq!(parse_dhcp_remaining(html, "Lease:"), Some(303));
        assert_eq!(parse_dhcp_time(html, "Lease:"), Some(600));
    }

    #[test]
    fn test_parse_downstream_qam() {
        let html = r#"<tr><td>Downstream 1</td><td>3</td><td>570.00 MHz</td><td>2.20 dBmV</td><td>40.37 dB</td><td>256QAM</td><td>2858</td><td>12</td><td>0</td></tr>"#;
        let channels = parse_downstream_qam(html);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].dcid, 3);
        assert!((channels[0].power_dbmv - 2.20).abs() < 0.01);
    }

    #[test]
    fn test_parse_uptime() {
        let html = r#"System Uptime: </td><td>0 d:  5 h: 34 m</td>"#;
        assert_eq!(parse_uptime(html), Some(5 * 3600 + 34 * 60));
    }

    #[test]
    fn test_parse_negative_power() {
        let html = r#"<tr><td>Downstream 17</td><td>18</td><td>698.00 MHz</td><td>-2.50 dBmV</td><td>36.34 dB</td><td>64QAM</td><td>370</td><td>11</td><td>0</td></tr>"#;
        let channels = parse_downstream_qam(html);
        assert_eq!(channels.len(), 1);
        assert!((channels[0].power_dbmv - (-2.50)).abs() < 0.01);
    }

    #[test]
    fn test_parse_event_log() {
        let html = r#"<td align=center>6/2/2026 7:11</td><td align=center>82000200</td><td align=center>3</td><td align=left>No Ranging Response received - T3 time-out;CM-MAC=aa:bb:cc:dd:ee:ff</td>"#;
        let events = parse_event_log(html);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "82000200");
        assert_eq!(events[0].event_level, 3);
        assert!(events[0].description.contains("T3 time-out"));
    }

    #[test]
    fn test_parse_cpe_counts() {
        let html = r#"Computers Detected:</td><td>staticCPE(0), dynamicCPE(1)</td>"#;
        let (s, d) = parse_cpe_counts(html);
        assert_eq!(s, 0);
        assert_eq!(d, 1);
    }

    #[test]
    fn test_parse_dhcp_state() {
        let html = r#"CM Dhcp State : </td> <td> bound</td>"#;
        assert_eq!(parse_dhcp_state(html), "bound");
    }

    #[test]
    fn test_parse_service_flow_configs() {
        let html = r#"
            UpstreamServiceFlow =
                SfClassName = ""
                SfMaxTrafficRate = 128000
                SfMaxTrafficBurst = 3044
                SfMinReservedRate = 0
                SfTrafficPriority = 1
                SfSchedulingType  = 2
            DownstreamServiceFlow =
                SfMaxTrafficRate = 128000
                SfMaxTrafficBurst = 3044
                SfMinReservedRate = 0
                SfTrafficPriority = 1
            DownstreamServiceFlow =
                SfMaxTrafficRate = 1126400
                SfMaxTrafficBurst = 3044
                SfMinReservedRate = 0
                SfTrafficPriority = 1
        "#;
        let flows = parse_service_flow_configs(html);
        assert_eq!(flows.len(), 3);
        assert_eq!(flows[0].direction, "Upstream");
        assert_eq!(flows[0].max_traffic_rate_kbps, 128000);
        assert_eq!(flows[0].scheduling_type, 2);
        assert_eq!(flows[1].direction, "Downstream");
        assert_eq!(flows[1].max_traffic_rate_kbps, 128000);
        assert_eq!(flows[2].max_traffic_rate_kbps, 1126400);
    }
}
