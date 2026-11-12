use regex::Regex;
use scraper::{Html, Selector};
use std::fmt;

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
    pub first_active_subcarrier_mhz: f64,
    pub last_active_subcarrier_mhz: f64,
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
    pub first_active_subcarrier_mhz: f64,
    pub last_active_subcarrier_mhz: f64,
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

#[derive(Debug, Clone, Default)]
pub struct VersionInfo {
    pub system: String,
    pub hardware_rev: String,
    pub vendor: String,
    pub bootloader: String,
    pub software_rev: String,
    pub model: String,
    pub serial: String,
    pub firmware_name: String,
    pub firmware_build_time: String,
}

#[derive(Debug, Clone, Default)]
pub struct DhcpInfo {
    pub cm_ip: String,
    pub cm_subnet: String,
    pub cm_gateway: String,
    pub tftp_server: String,
    pub time_server: String,
    pub time_offset_seconds: Option<i64>,
    pub boot_file: String,
    pub software_filename: String,
    pub mdd_ip_override: Option<u32>,
    pub mdd_ip_override_enabled: Option<bool>,
    pub mdd_ip_mode: Option<u32>,
    pub mdd_ip_mode_name: String,
    pub lease_total_secs: Option<u64>,
    pub lease_remaining_secs: Option<u64>,
    #[allow(dead_code)]
    pub rebind_total_secs: Option<u64>,
    pub rebind_remaining_secs: Option<u64>,
    #[allow(dead_code)]
    pub renew_total_secs: Option<u64>,
    pub renew_remaining_secs: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct CmState {
    pub overall_state: String,
    pub phases: Vec<DocsisPhase>,
    pub bpi_status: String,
    pub tod_status: String,
    pub dhcp_ipv4_attempts: Option<u32>,
    pub dhcp_ipv6_attempts: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub timestamp: String,
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
    pub speed_mbps: String,
    pub mac_address: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProductInfo {
    pub ethernet_phy_type: String,
    pub logging_components_enabled: Vec<(String, u32)>,
    pub logging_components: Vec<LoggingComponent>,
    pub severity_levels: Vec<SeverityLevel>,
}

#[derive(Debug, Clone)]
pub struct SpectrumChunk {
    pub chunk: usize,
    pub center_freq_hz: i64,
    pub span_hz: i64,
    pub bin_spacing_hz: i64,
    pub resolution_bandwidth: i64,
    pub bins_raw_tenth_dbmv: Vec<i16>,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceFlowConfig {
    pub direction: String,
    pub index: u32,
    pub reference: u32,
    pub sf_id: Option<u64>,
    pub service_class: String,
    pub qos_set_type: u32,
    pub max_traffic_rate_bps: u64,
    pub max_traffic_burst: u64,
    pub min_reserved_rate_bps: u64,
    pub min_reserved_rate_pktsize: u32,
    pub active_qos_timeout: u32,
    pub admitted_qos_timeout: u32,
    pub max_concat_burst: u32,
    pub traffic_priority: u32,
    pub scheduling_type: u32,
}

#[derive(Debug, Clone)]
pub struct PacketClassification {
    pub direction: String,
    pub index: u32,
    pub service_flow: u64,
    pub identifier: u32,
    pub rule_priority: u32,
    pub activation_state: u32,
    pub ip_protocol: u32,
    pub source_port_start: Option<u32>,
    pub source_port_end: Option<u32>,
    pub dest_port_start: Option<u32>,
    pub dest_port_end: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct CommonConfig {
    pub config_file: String,
    pub network_access: Option<bool>,
    pub upgrade_server: String,
    pub max_cpe_allowed: Option<u32>,
    pub privacy_enable: Option<u32>,
    pub snmp_mib: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StatusInfo {
    pub time_and_date: String,
    pub product_type: String,
    pub hardware_info: String,
    pub router_enabled: Option<bool>,
    pub telemetry_enabled: Option<bool>,
    pub rf_hw: String,
    pub market: String,
    pub l2vpn_attachment_circuit_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DiplexerBand {
    pub band: u32,
    pub upstream_range: String,
    pub downstream_range: String,
    pub current: bool,
}

#[derive(Debug, Clone)]
pub struct LoggingComponent {
    pub index: u32,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SeverityLevel {
    pub index: u32,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DhcpLogEntry {
    pub index: u32,
    pub timestamp: String,
    pub interface: String,
    pub message_type: String,
    pub client_id: String,
    pub server_id: String,
    pub preference: Option<i32>,
    pub ia_type: Option<u32>,
    pub ia_id: Option<i64>,
    pub t1: Option<u64>,
    pub t2: Option<u64>,
    pub ia_addr_count: Option<u32>,
    pub stateful_addr: String,
    pub preferred_lifetime: Option<u64>,
    pub valid_lifetime: Option<u64>,
    pub options_len: Option<u32>,
    pub options: Vec<(u32, String)>,
}

#[derive(Debug, Clone)]
pub struct DsxLogEntry {
    pub cells: Vec<String>,
}

/// Per-section parse indicators. A `false` value means the section was present
/// in the response but its expected structure could not be parsed.
#[derive(Debug, Clone, Default)]
pub struct ParseStatus {
    pub status: bool,
    pub channels: bool,
    pub version: bool,
    pub dhcp: bool,
    pub qos: bool,
    pub cm_state: bool,
    pub event_log: bool,
    pub interfaces: bool,
    pub product: bool,
    pub service_flow_config: bool,
    pub dhcp_log: bool,
    pub dsx_log: bool,
    pub spectrum: Option<bool>,
}

impl ParseStatus {
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, bool)> + '_ {
        [
            ("status", self.status),
            ("channels", self.channels),
            ("version", self.version),
            ("dhcp", self.dhcp),
            ("qos", self.qos),
            ("cm_state", self.cm_state),
            ("event_log", self.event_log),
            ("interfaces", self.interfaces),
            ("product", self.product),
            ("service_flow_config", self.service_flow_config),
            ("dhcp_log", self.dhcp_log),
            ("dsx_log", self.dsx_log),
        ]
        .into_iter()
        .chain(self.spectrum.map(|ok| ("spectrum", ok)))
    }
}

/// Returned when the modem responses are structurally valid HTTP but do not
/// contain the fields the exporter requires. Carries the per-section status so
/// callers can still expose which sections did parse.
#[derive(Debug, Clone)]
pub struct ParseFailure {
    pub missing: Vec<&'static str>,
    pub status: ParseStatus,
}

impl fmt::Display for ParseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "essential modem fields missing or unparsable: {}",
            self.missing.join(", ")
        )
    }
}

impl std::error::Error for ParseFailure {}

#[derive(Debug, Clone)]
pub struct ScrapedData {
    pub downstream_qam: Vec<DownstreamQam>,
    pub downstream_ofdm: Vec<DownstreamOfdm>,
    pub upstream_qam: Vec<UpstreamQam>,
    pub upstream_ofdm: Vec<UpstreamOfdm>,
    pub uptime_seconds: Option<u64>,
    pub cm_status: String,
    pub status_info: StatusInfo,
    pub version_info: VersionInfo,
    pub dhcp_info: DhcpInfo,
    pub qos_flows: Vec<QosFlow>,
    pub cm_state: CmState,
    pub events: Vec<EventLogEntry>,
    pub interfaces: Vec<InterfaceInfo>,
    pub product_info: ProductInfo,
    pub spectrum: Vec<SpectrumChunk>,
    pub cpe_static: u32,
    pub cpe_dynamic: u32,
    pub dhcp_state: String,
    pub service_flow_configs: Vec<ServiceFlowConfig>,
    pub packet_classifications: Vec<PacketClassification>,
    pub common_config: CommonConfig,
    pub diplexer_bands: Vec<DiplexerBand>,
    pub dhcp_log: Vec<DhcpLogEntry>,
    pub dsx_log: Vec<DsxLogEntry>,
    pub scrape_duration_secs: f64,
    pub parse_status: ParseStatus,
}

#[derive(Debug, Default)]
struct ChannelData {
    downstream_qam: Vec<DownstreamQam>,
    downstream_ofdm: Vec<DownstreamOfdm>,
    upstream_qam: Vec<UpstreamQam>,
    upstream_ofdm: Vec<UpstreamOfdm>,
}

impl ChannelData {
    fn is_empty(&self) -> bool {
        self.downstream_qam.is_empty()
            && self.downstream_ofdm.is_empty()
            && self.upstream_qam.is_empty()
            && self.upstream_ofdm.is_empty()
    }
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
    product_html: &str,
    dhcp_log_html: &str,
    dsx_html: &str,
    spectrum_html: Option<&str>,
    scrape_duration_secs: f64,
) -> std::result::Result<ScrapedData, ParseFailure> {
    let channels = parse_channels(status_html);
    let (cpe_static, cpe_dynamic) = parse_cpe_counts(status_html);
    let uptime_seconds = parse_uptime(status_html);
    let cm_status = parse_cm_status(status_html);
    let status_info = parse_status_info(status_html);
    let version_info = parse_version_info(vers_html, status_html);
    let dhcp_info = parse_dhcp_info(dhcp_html);
    let dhcp_state = parse_dhcp_state(dhcp_html);
    let qos_flows = parse_qos(qos_html);
    let cm_state = parse_cm_state(cm_state_html);
    let events = parse_event_log(event_html);
    let interfaces = parse_interfaces(status_html);
    let product_info = parse_product_info(status_html, product_html);
    let spectrum = spectrum_html.map(parse_spectrum_chunks).unwrap_or_default();
    let service_flow_configs = parse_service_flow_configs(config_params_html);
    let packet_classifications = parse_packet_classifications(config_params_html);
    let common_config = parse_common_config(config_params_html);
    let diplexer_bands = parse_diplexer_bands(status_html);
    let dhcp_log = parse_dhcp_log(dhcp_log_html);
    let dsx_log = parse_dsx_log(dsx_html);

    let parse_status = ParseStatus {
        status: !cm_status.is_empty(),
        channels: !channels.is_empty()
            || (!cm_status.is_empty() && !cm_status.eq_ignore_ascii_case("OPERATIONAL")),
        version: !version_info.model.is_empty(),
        dhcp: !dhcp_state.is_empty() || !dhcp_info.cm_ip.is_empty(),
        qos: !qos_flows.is_empty() || qos_html.contains("SFID"),
        cm_state: !cm_state.overall_state.is_empty() || !cm_state.phases.is_empty(),
        event_log: !events.is_empty() || event_html.contains("Event ID"),
        interfaces: !interfaces.is_empty() || status_html.contains("Interface Parameters"),
        product: !product_info.ethernet_phy_type.is_empty()
            || !product_info.logging_components_enabled.is_empty(),
        service_flow_config: !service_flow_configs.is_empty()
            || config_params_html.contains("ServiceFlow"),
        dhcp_log: !dhcp_log.is_empty() || dhcp_log_html.contains("CM DHCP LOG"),
        dsx_log: !dsx_log.is_empty() || dsx_html.contains("DSX Message Log"),
        spectrum: spectrum_html.map(|html| html.contains("spectrum_data") && !spectrum.is_empty()),
    };

    let mut missing = Vec::new();
    if !parse_status.status {
        missing.push("status");
    }
    if !parse_status.version {
        missing.push("version/model");
    }
    // A modem that is not operational legitimately has no bonded channels.
    // When it claims to be operational, an empty channel set means the page
    // structure changed or only part of the response was returned.
    if cm_status.eq_ignore_ascii_case("OPERATIONAL") && !parse_status.channels {
        missing.push("channels");
    }
    if !missing.is_empty() {
        return Err(ParseFailure {
            missing,
            status: parse_status,
        });
    }

    Ok(ScrapedData {
        downstream_qam: channels.downstream_qam,
        downstream_ofdm: channels.downstream_ofdm,
        upstream_qam: channels.upstream_qam,
        upstream_ofdm: channels.upstream_ofdm,
        uptime_seconds,
        cm_status,
        status_info,
        version_info,
        dhcp_info,
        dhcp_state,
        qos_flows,
        cm_state,
        events,
        interfaces,
        product_info,
        spectrum,
        cpe_static,
        cpe_dynamic,
        service_flow_configs,
        packet_classifications,
        common_config,
        diplexer_bands,
        dhcp_log,
        dsx_log,
        scrape_duration_secs,
        parse_status,
    })
}

/// Parse channel rows through the HTML tree instead of matching raw markup.
/// The modem's firmware emits the same cells with varying whitespace and
/// attributes, which made the previous regular expressions brittle.
fn parse_channels(html: &str) -> ChannelData {
    let mut data = parse_channel_rows(html);
    if data.is_empty() {
        // Unit tests and callers may supply a bare <tr> fragment. HTML5
        // parsing drops table rows that are not inside a table context.
        let wrapped = format!("<table>{}</table>", html);
        data = parse_channel_rows(&wrapped);
    }
    data
}

fn parse_channel_rows(html: &str) -> ChannelData {
    let document = Html::parse_document(html);
    let Ok(row_selector) = Selector::parse("tr") else {
        return ChannelData::default();
    };
    let Ok(cell_selector) = Selector::parse("td") else {
        return ChannelData::default();
    };

    let mut data = ChannelData::default();
    for row in document.select(&row_selector) {
        let cells: Vec<String> = row
            .select(&cell_selector)
            .map(|cell| cell.text().collect::<String>().trim().to_string())
            .collect();
        if cells.len() < 2 {
            continue;
        }

        if let Some(channel) = parse_downstream_row(&cells) {
            data.downstream_qam.push(channel);
        } else if let Some(channel) = parse_downstream_ofdm_row(&cells) {
            data.downstream_ofdm.push(channel);
        } else if let Some(channel) = parse_upstream_row(&cells) {
            data.upstream_qam.push(channel);
        } else if let Some(channel) = parse_upstream_ofdm_row(&cells) {
            data.upstream_ofdm.push(channel);
        }
    }
    data
}

fn channel_number(cell: &str, direction: &str) -> Option<u32> {
    let rest = cell.strip_prefix(direction)?.trim();
    rest.parse().ok()
}

fn parse_downstream_row(cells: &[String]) -> Option<DownstreamQam> {
    if cells.len() < 9 {
        return None;
    }
    let channel = channel_number(&cells[0], "Downstream")?;
    // OFDM rows use a channel width such as "4K" in this position.
    if !cells[1].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !cells[2].ends_with("MHz") || cells[5].len() < 3 {
        return None;
    }
    Some(DownstreamQam {
        channel,
        dcid: cells[1].parse().ok()?,
        freq_mhz: number_with_unit(&cells[2], "MHz")?,
        power_dbmv: number_with_unit(&cells[3], "dBmV")?,
        snr_db: number_with_unit(&cells[4], "dB")?,
        modulation: cells[5].clone(),
        octets: cells[6].parse().ok()?,
        correcteds: cells[7].parse().ok()?,
        uncorrectables: cells[8].parse().ok()?,
    })
}

fn parse_downstream_ofdm_row(cells: &[String]) -> Option<DownstreamOfdm> {
    if cells.len() < 9 {
        return None;
    }
    let channel = channel_number(&cells[0], "Downstream")?;
    if !cells[1].ends_with('K') {
        return None;
    }
    Some(DownstreamOfdm {
        channel,
        fft_type: cells[1].clone(),
        channel_width_mhz: cells[2].parse().ok()?,
        active_subcarriers: cells[3].parse().ok()?,
        first_active_subcarrier_mhz: cells[4].parse().ok()?,
        last_active_subcarrier_mhz: cells[5].parse().ok()?,
        rxmer_pilot_db: cells[6].parse().ok()?,
        rxmer_plc_db: cells[7].parse().ok()?,
        rxmer_data_db: cells[8].parse().ok()?,
    })
}

fn parse_upstream_row(cells: &[String]) -> Option<UpstreamQam> {
    if cells.len() < 7 {
        return None;
    }
    let channel = channel_number(&cells[0], "Upstream")?;
    if !cells[1].chars().all(|c| c.is_ascii_digit()) || !cells[5].contains("kSym/s") {
        return None;
    }
    Some(UpstreamQam {
        channel,
        ucid: cells[1].parse().ok()?,
        freq_mhz: number_with_unit(&cells[2], "MHz")?,
        power_dbmv: number_with_unit(&cells[3], "dBmV")?,
        channel_type: cells[4].clone(),
        symbol_rate_ksym: number_with_unit(&cells[5], "kSym/s")?,
        modulation: cells[6].clone(),
    })
}

fn parse_upstream_ofdm_row(cells: &[String]) -> Option<UpstreamOfdm> {
    if cells.len() < 9 {
        return None;
    }
    let channel = channel_number(&cells[0], "Upstream")?;
    if !cells[1].ends_with('K') {
        return None;
    }
    Some(UpstreamOfdm {
        channel,
        fft_type: cells[1].clone(),
        channel_width_mhz: cells[2].parse().ok()?,
        active_subcarriers: cells[3].parse().ok()?,
        first_active_subcarrier_mhz: cells[4].parse().ok()?,
        last_active_subcarrier_mhz: cells[5].parse().ok()?,
        tx_power_dbmv: cells[8].parse().ok()?,
    })
}

fn number_with_unit(cell: &str, unit: &str) -> Option<f64> {
    cell.trim().strip_suffix(unit)?.trim().parse().ok()
}

fn table_rows(html: &str) -> Vec<Vec<String>> {
    let document = Html::parse_document(html);
    let Ok(row_selector) = Selector::parse("tr") else {
        return Vec::new();
    };
    let Ok(cell_selector) = Selector::parse("td") else {
        return Vec::new();
    };
    document
        .select(&row_selector)
        .map(|row| {
            row.select(&cell_selector)
                .map(|cell| cell.text().collect::<String>().trim().to_string())
                .collect()
        })
        .filter(|cells: &Vec<String>| !cells.is_empty())
        .collect()
}

fn normalize_label(label: &str) -> String {
    label
        .trim()
        .trim_end_matches(':')
        .trim()
        .to_ascii_lowercase()
}

fn table_value(rows: &[Vec<String>], label: &str) -> Option<String> {
    let wanted = normalize_label(label);
    rows.iter().find_map(|row| {
        if row.len() >= 2 && normalize_label(&row[0]) == wanted {
            let value = row[1].trim();
            (!value.is_empty()).then(|| value.to_string())
        } else {
            None
        }
    })
}

fn enabled_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "enabled" | "yes" | "on" | "true" => Some(true),
        "disabled" | "no" | "off" | "false" => Some(false),
        _ => None,
    }
}

fn parse_status_info(html: &str) -> StatusInfo {
    let rows = table_rows(html);
    let product_type = Regex::new(r"Product Type:\s*</b>\s*([^<\n]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let l2vpn_attachment_circuit_ids = rows
        .iter()
        .filter(|row| {
            row.len() >= 2
                && normalize_label(&row[0]).starts_with("attachment circuit id")
                && !row[1].trim().is_empty()
        })
        .map(|row| row[1].trim().to_string())
        .collect();

    StatusInfo {
        time_and_date: table_value(&rows, "Time and Date").unwrap_or_default(),
        product_type,
        hardware_info: table_value(&rows, "Hardware Info").unwrap_or_default(),
        router_enabled: table_value(&rows, "Router").and_then(|v| enabled_value(&v)),
        telemetry_enabled: table_value(&rows, "Telemetry").and_then(|v| enabled_value(&v)),
        rf_hw: table_value(&rows, "RF HW").unwrap_or_default(),
        market: table_value(&rows, "Market").unwrap_or_default(),
        l2vpn_attachment_circuit_ids,
    }
}

fn parse_diplexer_bands(html: &str) -> Vec<DiplexerBand> {
    table_rows(html)
        .into_iter()
        .filter_map(|row| {
            if row.len() < 4 {
                return None;
            }
            let band = row[0].strip_prefix("Band")?.trim().parse().ok()?;
            Some(DiplexerBand {
                band,
                upstream_range: row[1].trim().to_string(),
                downstream_range: row[2].trim().to_string(),
                current: row[3].trim().eq_ignore_ascii_case("X"),
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
    let system = Regex::new(r"System:\s*</td>\s*<td[^>]*>([^<]+)")
        .unwrap()
        .captures(vers_html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

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
        system,
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

    let rows = table_rows(html);
    let mdd_override_raw = table_value(&rows, "MDD IP Override").unwrap_or_default();
    let mdd_mode_raw = table_value(&rows, "MDD IP Mode").unwrap_or_default();
    let offset_raw = table_value(&rows, "CM Time Offset").unwrap_or_default();

    DhcpInfo {
        cm_ip,
        cm_subnet,
        cm_gateway,
        tftp_server: table_value(&rows, "CM TFTP Server").unwrap_or_default(),
        time_server: table_value(&rows, "CM Time Server").unwrap_or_default(),
        time_offset_seconds: offset_raw.parse().ok(),
        boot_file: table_value(&rows, "CM Boot file").unwrap_or_default(),
        software_filename: table_value(&rows, "Software Filename").unwrap_or_default(),
        mdd_ip_override: leading_u32(&mdd_override_raw),
        mdd_ip_override_enabled: parenthesized_enabled(&mdd_override_raw),
        mdd_ip_mode: leading_u32(&mdd_mode_raw),
        mdd_ip_mode_name: parenthesized_value(&mdd_mode_raw).unwrap_or_default(),
        lease_total_secs,
        lease_remaining_secs,
        rebind_total_secs,
        rebind_remaining_secs,
        renew_total_secs,
        renew_remaining_secs,
    }
}

fn leading_u32(value: &str) -> Option<u32> {
    value
        .split_whitespace()
        .next()
        .and_then(|token| token.parse().ok())
}

fn parenthesized_value(value: &str) -> Option<String> {
    let start = value.find('(')?;
    let end = value.rfind(')')?;
    (end > start).then(|| value[start + 1..end].trim().to_string())
}

fn parenthesized_enabled(value: &str) -> Option<bool> {
    parenthesized_value(value).and_then(|v| enabled_value(&v))
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

    let dhcp_ipv4_attempts = parse_attempts(html, "IPv4 Attempt\\(s\\)");
    let dhcp_ipv6_attempts = parse_attempts(html, "IPv6 Attempt\\(s\\)");

    CmState {
        overall_state,
        phases,
        bpi_status,
        tod_status,
        dhcp_ipv4_attempts,
        dhcp_ipv6_attempts,
    }
}

fn parse_attempts(html: &str, label: &str) -> Option<u32> {
    let pattern = format!(r"{}\s*</td>\s*<td[^>]*>\s*(\d+)", label);
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .and_then(|c| c[1].parse().ok())
}

fn parse_event_log(html: &str) -> Vec<EventLogEntry> {
    let re = Regex::new(
        r#"<td\s+align=center>([^<]+)</td>\s*<td\s+align=center>(\d+)</td>\s*<td\s+align=center>(\d+)</td>\s*<td\s+align=left>([^<]+)</td>"#
    ).unwrap();

    re.captures_iter(html)
        .filter_map(|c| {
            Some(EventLogEntry {
                timestamp: c[1].trim().to_string(),
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
                speed_mbps: c[4].trim().to_string(),
                mac_address: c[5].trim().to_string(),
            })
        })
        .collect()
}

fn parse_product_info(status_html: &str, product_html: &str) -> ProductInfo {
    let ethernet_phy_type = Regex::new(r"Ethernet Phy Type</td><td>([^<]+)")
        .unwrap()
        .captures(status_html)
        .or_else(|| {
            Regex::new(r"Ethernet Phy Type</td><td>([^<]+)")
                .unwrap()
                .captures(product_html)
        })
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let mut logging_components_enabled = Vec::new();
    let group_re = Regex::new(
        r#"<br><b>\[(\d+)\]\s*([^<]+)</b>:\s*Enabled\s*<br>\s*<table[^>]*>(.*?)</table>"#,
    )
    .unwrap();
    let enabled_re = Regex::new(r": Enabled").unwrap();
    for caps in group_re.captures_iter(product_html) {
        let group = caps[2].trim().to_string();
        let count = enabled_re.find_iter(&caps[3]).count() as u32;
        logging_components_enabled.push((group, count));
    }

    let (component_html, severity_html) = product_html
        .split_once("Severity Levels")
        .unwrap_or((product_html, ""));
    let logging_components = parse_enabled_cells(component_html)
        .into_iter()
        .map(|(index, name, enabled)| LoggingComponent {
            index,
            name,
            enabled,
        })
        .collect();
    let severity_levels = parse_enabled_cells(severity_html)
        .into_iter()
        .map(|(index, name, enabled)| SeverityLevel {
            index,
            name,
            enabled,
        })
        .collect();

    ProductInfo {
        ethernet_phy_type,
        logging_components_enabled,
        logging_components,
        severity_levels,
    }
}

fn parse_enabled_cells(html: &str) -> Vec<(u32, String, bool)> {
    let re = Regex::new(r"\[\s*(\d+)\]\s*([A-Za-z0-9_]+)\s*:\s*(Enabled|Disabled)").unwrap();
    table_rows(html)
        .into_iter()
        .flatten()
        .filter_map(|cell| {
            let caps = re.captures(&cell)?;
            Some((
                caps[1].parse().ok()?,
                caps[2].to_string(),
                caps[3].eq_ignore_ascii_case("Enabled"),
            ))
        })
        .collect()
}

fn parse_spectrum_chunks(html: &str) -> Vec<SpectrumChunk> {
    let data_re = Regex::new(r#"var\s+spectrum_data\s*=\s*\[(.*?)\];"#).unwrap();
    let chunk_re = Regex::new(r#"\"([0-9a-fA-F$]+)\""#).unwrap();
    let Some(data_caps) = data_re.captures(html) else {
        return Vec::new();
    };

    chunk_re
        .captures_iter(&data_caps[1])
        .enumerate()
        .filter_map(|(idx, caps)| parse_spectrum_chunk(idx, &caps[1]))
        .collect()
}

fn parse_spectrum_chunk(chunk: usize, raw: &str) -> Option<SpectrumChunk> {
    let mut pos = usize::from(raw.starts_with('$'));
    let get2 = |pos: &mut usize| -> Option<i16> {
        let v = u16::from_str_radix(raw.get(*pos..*pos + 4)?, 16).ok()?;
        *pos += 4;
        Some(v as i16)
    };
    let get4 = |pos: &mut usize| -> Option<i64> {
        let v = u32::from_str_radix(raw.get(*pos..*pos + 8)?, 16).ok()?;
        *pos += 8;
        Some((v as i32) as i64)
    };

    let center_freq_hz = get4(&mut pos)?;
    let span_hz = get4(&mut pos)?;
    let bins = usize::try_from(get4(&mut pos)?).ok()?;
    let bin_spacing_hz = get4(&mut pos)?;
    let resolution_bandwidth = get4(&mut pos)?;
    let mut bins_raw_tenth_dbmv = Vec::with_capacity(bins);
    for _ in 0..bins {
        bins_raw_tenth_dbmv.push(get2(&mut pos)?);
    }

    Some(SpectrumChunk {
        chunk,
        center_freq_hz,
        span_hz,
        bin_spacing_hz,
        resolution_bandwidth,
        bins_raw_tenth_dbmv,
    })
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
    let dir_re = Regex::new(r"(Upstream|Downstream)ServiceFlow\s*=").unwrap();
    let starts: Vec<(usize, usize, String)> = dir_re
        .captures_iter(html)
        .filter_map(|cap| {
            let full = cap.get(0)?;
            Some((full.start(), full.end(), cap[1].to_string()))
        })
        .collect();

    starts
        .iter()
        .enumerate()
        .map(|(index, (_, start, direction))| {
            let end = starts
                .get(index + 1)
                .map(|(next_start, _, _)| *next_start)
                .unwrap_or(html.len());
            let chunk = &html[*start..end];
            ServiceFlowConfig {
                direction: direction.clone(),
                index: index as u32,
                reference: field_number(chunk, "SfReference").unwrap_or(0),
                sf_id: Regex::new(r"#\s*SfId\s*=\s*(\d+)")
                    .unwrap()
                    .captures(chunk)
                    .and_then(|c| c[1].parse().ok()),
                service_class: Regex::new(r#"SfClassName\s*=\s*"([^"]*)""#)
                    .unwrap()
                    .captures(chunk)
                    .map(|c| c[1].to_string())
                    .unwrap_or_default(),
                qos_set_type: field_number(chunk, "SfQosSetType").unwrap_or(0),
                max_traffic_rate_bps: field_number(chunk, "SfMaxTrafficRate").unwrap_or(0),
                max_traffic_burst: field_number(chunk, "SfMaxTrafficBurst").unwrap_or(0),
                min_reserved_rate_bps: field_number(chunk, "SfMinReservedRate").unwrap_or(0),
                min_reserved_rate_pktsize: field_number(chunk, "SfMinReservedRatePktsize")
                    .unwrap_or(0),
                active_qos_timeout: field_number(chunk, "SfActiveQosTimeout").unwrap_or(0),
                admitted_qos_timeout: field_number(chunk, "SfAdmittedQosTimeout").unwrap_or(0),
                max_concat_burst: field_number(chunk, "SfMaxConcatBurst").unwrap_or(0),
                traffic_priority: field_number(chunk, "SfTrafficPriority").unwrap_or(0),
                scheduling_type: field_number(chunk, "SfSchedulingType").unwrap_or(0),
            }
        })
        .collect()
}

fn field_number<T: std::str::FromStr>(chunk: &str, name: &str) -> Option<T> {
    let pattern = format!(r"{}\s*=\s*(-?\d+)", regex::escape(name));
    Regex::new(&pattern)
        .ok()?
        .captures(chunk)
        .and_then(|c| c[1].parse().ok())
}

fn parse_packet_classifications(html: &str) -> Vec<PacketClassification> {
    let re = Regex::new(r"(Upstream|Downstream)PacketClassification\s*=").unwrap();
    let starts: Vec<(usize, usize, String)> = re
        .captures_iter(html)
        .filter_map(|cap| {
            let full = cap.get(0)?;
            Some((full.start(), full.end(), cap[1].to_string()))
        })
        .collect();

    starts
        .iter()
        .enumerate()
        .map(|(index, (_, start, direction))| {
            let end = starts
                .get(index + 1)
                .map(|(next_start, _, _)| *next_start)
                .unwrap_or(html.len());
            let chunk = &html[*start..end];
            PacketClassification {
                direction: direction.clone(),
                index: index as u32,
                service_flow: field_number(chunk, "PcServiceFlow").unwrap_or(0),
                identifier: field_number(chunk, "PcIdentifier").unwrap_or(0),
                rule_priority: field_number(chunk, "PcRulePriority").unwrap_or(0),
                activation_state: field_number(chunk, "PcActivationState").unwrap_or(0),
                ip_protocol: field_number(chunk, "PcIpProtocol").unwrap_or(0),
                source_port_start: field_number(chunk, "PcIpSourcePortStart"),
                source_port_end: field_number(chunk, "PcIpSourcePortEnd"),
                dest_port_start: field_number(chunk, "PcIpDestPortStart"),
                dest_port_end: field_number(chunk, "PcIpDestPortEnd"),
            }
        })
        .collect()
}

fn parse_common_config(html: &str) -> CommonConfig {
    let config_file = Regex::new(r"CM Config File:\s*</td>\s*<td[^>]*>\s*([^<\s]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let network_access = Regex::new(r"NetworkAccess\s*=\s*([A-Za-z]+)")
        .unwrap()
        .captures(html)
        .and_then(|c| enabled_value(&c[1]));
    let upgrade_server = Regex::new(r"UpgradeServer\s*=\s*([^\s<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let max_cpe_allowed = field_number(html, "MaxCpeAllowed");
    let privacy_enable = field_number(html, "PrivacyEnable");
    let snmp_mib = Regex::new(r"SnmpMib\s*=\s*([^\n<]+)")
        .unwrap()
        .captures_iter(html)
        .map(|c| c[1].trim().to_string())
        .collect();

    CommonConfig {
        config_file,
        network_access,
        upgrade_server,
        max_cpe_allowed,
        privacy_enable,
        snmp_mib,
    }
}

fn parse_dhcp_log(html: &str) -> Vec<DhcpLogEntry> {
    let block_re = Regex::new(r"(?s)CM DHCP LOG\s+(\d+)<br>\s*([^<]*)<br>(.*?)</FONT>").unwrap();
    block_re
        .captures_iter(html)
        .filter_map(|caps| {
            let index = caps[1].parse().ok()?;
            let timestamp = caps[2].trim().to_string();
            let chunk = &caps[3];
            let ia_addr =
                Regex::new(r"Stateful addr:\s*([^,<]+),\s*Pltime:\s*(\d+),\s*Vltime:\s*(\d+)")
                    .unwrap()
                    .captures(chunk);
            let options = Regex::new(r"Option:\s*\[(\d+)\]([^,<]+),\s*option len:\s*(\d+)")
                .unwrap()
                .captures_iter(chunk)
                .filter_map(|c| Some((c[1].parse().ok()?, c[2].trim().to_string())))
                .collect();

            Some(DhcpLogEntry {
                index,
                timestamp,
                interface: field_text(chunk, "Interface"),
                message_type: field_text(chunk, "Message type"),
                client_id: field_text(chunk, "Client ID"),
                server_id: field_text(chunk, "Server ID"),
                preference: field_number_colon(chunk, "Preference"),
                ia_type: field_number_colon(chunk, "IA type"),
                ia_id: field_number_colon(chunk, "IA ID"),
                t1: field_number_colon(chunk, "t1"),
                t2: field_number_colon(chunk, "t2"),
                ia_addr_count: field_number_colon(chunk, "IA addr count"),
                stateful_addr: ia_addr
                    .as_ref()
                    .map(|c| c[1].trim().to_string())
                    .unwrap_or_default(),
                preferred_lifetime: ia_addr.as_ref().and_then(|c| c[2].parse().ok()),
                valid_lifetime: ia_addr.as_ref().and_then(|c| c[3].parse().ok()),
                options_len: field_number_colon(chunk, "Options buffer: len"),
                options,
            })
        })
        .collect()
}

fn field_number_colon<T: std::str::FromStr>(chunk: &str, name: &str) -> Option<T> {
    let pattern = format!(r"{}\s*:\s*(-?\d+)", regex::escape(name));
    Regex::new(&pattern)
        .ok()?
        .captures(chunk)
        .and_then(|c| c[1].parse().ok())
}

fn field_text(chunk: &str, name: &str) -> String {
    let pattern = format!(r"{}\s*:\s*([^<]*)", regex::escape(name));
    Regex::new(&pattern)
        .ok()
        .and_then(|re| re.captures(chunk))
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default()
}

fn parse_dsx_log(html: &str) -> Vec<DsxLogEntry> {
    table_rows(html)
        .into_iter()
        .filter(|row| {
            !row.is_empty()
                && !row[0].contains("DSX Message Log")
                && row.iter().any(|cell| !cell.trim().is_empty())
        })
        .map(|cells| DsxLogEntry { cells })
        .collect()
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
        let channels = parse_channels(html).downstream_qam;
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].dcid, 3);
        assert!((channels[0].power_dbmv - 2.20).abs() < 0.01);
    }

    #[test]
    fn test_channel_rows_tolerate_attributes_and_whitespace() {
        let html = r#"
            <table>
              <tr class="channel">
                <td data-label="channel"> Downstream  1 </td>
                <td data-label="dcid">3</td>
                <td>  570.00 MHz </td>
                <td> 2.20 dBmV </td>
                <td> 40.37 dB </td>
                <td> 256QAM </td>
                <td>2858</td><td>12</td><td>0</td>
              </tr>
              <tr><td>Downstream 1</td><td>4K</td><td>190</td><td>3800</td><td>135</td><td>324</td><td>49</td><td>43</td><td>43</td></tr>
            </table>
        "#;
        let channels = parse_channels(html);
        assert_eq!(channels.downstream_qam.len(), 1);
        assert_eq!(channels.downstream_ofdm.len(), 1);
        assert!((channels.downstream_qam[0].freq_mhz - 570.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_uptime() {
        let html = r#"System Uptime: </td><td>0 d:  5 h: 34 m</td>"#;
        assert_eq!(parse_uptime(html), Some(5 * 3600 + 34 * 60));
    }

    #[test]
    fn test_parse_negative_power() {
        let html = r#"<tr><td>Downstream 17</td><td>18</td><td>698.00 MHz</td><td>-2.50 dBmV</td><td>36.34 dB</td><td>64QAM</td><td>370</td><td>11</td><td>0</td></tr>"#;
        let channels = parse_channels(html).downstream_qam;
        assert_eq!(channels.len(), 1);
        assert!((channels[0].power_dbmv - (-2.50)).abs() < 0.01);
    }

    #[test]
    fn test_parse_event_log() {
        let html = r#"<td align=center>6/2/2026 7:11</td><td align=center>82000200</td><td align=center>3</td><td align=left>No Ranging Response received - T3 time-out;CM-MAC=aa:bb:cc:dd:ee:ff</td>"#;
        let events = parse_event_log(html);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].timestamp, "6/2/2026 7:11");
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
        assert_eq!(flows[0].max_traffic_rate_bps, 128000);
        assert_eq!(flows[0].scheduling_type, 2);
        assert_eq!(flows[1].direction, "Downstream");
        assert_eq!(flows[1].max_traffic_rate_bps, 128000);
        assert_eq!(flows[2].max_traffic_rate_bps, 1126400);
    }

    #[test]
    fn test_parse_upstream_ofdm_uses_last_numeric_field_as_power() {
        let html = r#"<tr><td>Upstream 0</td><td>2K</td><td>32.000000</td><td>640</td><td>74</td><td>773</td><td>29.8</td><td>64.8</td><td>37.000000</td></tr>"#;
        let channels = parse_channels(html).upstream_ofdm;
        assert_eq!(channels.len(), 1);
        assert!((channels[0].tx_power_dbmv - 37.0).abs() < 0.01);
        assert!((channels[0].first_active_subcarrier_mhz - 74.0).abs() < 0.01);
        assert!((channels[0].last_active_subcarrier_mhz - 773.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_spectrum_chunk() {
        let chunk = "1045a640007270e0000000040001c9c300000001fa24fa10fb64fbdc";
        let parsed = parse_spectrum_chunk(0, chunk).unwrap();
        assert_eq!(parsed.center_freq_hz, 273000000);
        assert_eq!(parsed.span_hz, 7500000);
        assert_eq!(parsed.bin_spacing_hz, 117187);
        assert_eq!(parsed.resolution_bandwidth, 1);
        assert_eq!(parsed.bins_raw_tenth_dbmv.len(), 4);
    }

    #[test]
    fn test_parse_downstream_ofdm_subcarrier_range() {
        let html = r#"<tr><td>Downstream 1</td><td>4K</td><td>190</td><td>3800</td><td>135</td><td>324</td><td>49</td><td>43</td><td>43</td></tr>"#;
        let channels = parse_channels(html).downstream_ofdm;
        assert_eq!(channels.len(), 1);
        assert!((channels[0].first_active_subcarrier_mhz - 135.0).abs() < 0.01);
        assert!((channels[0].last_active_subcarrier_mhz - 324.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_status_and_diplexer_details() {
        let html = r#"
        <table>
          <tr><td>Time and Date:</td><td>Thu 2026-11-12 16:08:07</td></tr>
          <tr><td>Hardware Info</td><td>ARRIS DOCSIS 3.1</td></tr>
          <tr><td>Router</td><td>DISABLED</td></tr>
          <tr><td>Telemetry</td><td>DISABLED</td></tr>
          <tr><td>RF HW</td><td>MAXLINEAR</td></tr>
          <tr><td>Market</td><td>Euro</td></tr>
        </table>
        Product Type:</b> 00121000 00008900
        <table>
          <tr><td>Band 0</td><td>5 MHz-85 MHz</td><td>108 MHz-1218 MHz</td><td>X</td></tr>
          <tr><td>Band 1</td><td>5 MHz-204 MHz</td><td>258 MHz-1218 MHz</td><td> </td></tr>
        </table>
        "#;
        let info = parse_status_info(html);
        assert_eq!(info.product_type, "00121000 00008900");
        assert_eq!(info.hardware_info, "ARRIS DOCSIS 3.1");
        assert_eq!(info.router_enabled, Some(false));
        assert_eq!(info.rf_hw, "MAXLINEAR");
        assert_eq!(info.market, "Euro");
        let bands = parse_diplexer_bands(html);
        assert_eq!(bands.len(), 2);
        assert!(bands[0].current);
        assert!(!bands[1].current);
    }

    #[test]
    fn test_parse_dhcp_log_entry() {
        let html = r#"
        <TABLE><TR><TD><FONT>CM DHCP LOG 3<br>
        Sun, 25 Oct 2026 09:26:54 +0100<br>Interface: wan0<br>Message type: renew<br>Client ID: aa:bb<br>Server ID: cc:dd<br>Preference: -1<br>IA_NA/IA_PD count: 1<br>IA details:<br>        IA type: 3, IA ID: -763769691, t1: 172800, t2: 483840, IA addr count: 1<br>        IA prefix addr details:<br>                Stateful addr: 2a02:8101:1130:d::c2a, Pltime: 604800, Vltime: 604800<br>Options buffer: len: 86, details:<br>        Option: [1]client ID, option len: 10<br>        Option: [3]identity association, option len: 40<br></FONT></TD></TR></TABLE>
        "#;
        let entries = parse_dhcp_log(html);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.index, 3);
        assert_eq!(entry.message_type, "renew");
        assert_eq!(entry.ia_id, Some(-763769691));
        assert_eq!(entry.stateful_addr, "2a02:8101:1130:d::c2a");
        assert_eq!(entry.preferred_lifetime, Some(604800));
        assert_eq!(entry.valid_lifetime, Some(604800));
        assert_eq!(entry.options_len, Some(86));
        assert_eq!(entry.options.len(), 2);
    }

    #[test]
    fn test_parse_service_flow_and_classification_details() {
        let html = r#"
            UpstreamServiceFlow =
                SfReference = 0
                # SfId = 99369107 -- assigned by CMTS
                SfClassName = "BE"
                SfQosSetType = 7
                SfMaxTrafficRate = 86250000
                SfMaxTrafficBurst = 65535
                SfMinReservedRate = 0
                SfMinReservedRatePktsize = 0
                SfActiveQosTimeout = 0
                SfAdmittedQosTimeout = 200
                SfMaxConcatBurst = 40000
                SfTrafficPriority = 0
                SfSchedulingType = 2
            UpstreamPacketClassification =
                PcServiceFlow = 99369110
                PcIdentifier = 48
                PcRulePriority = 129
                PcActivationState = 1
                PcIpProtocol = 17
                PcIpSourcePortStart = 0
                PcIpSourcePortEnd = 65535
            Common CM Config Parameters
                NetworkAccess = Yes
                UpgradeServer = 172.22.48.1
                MaxCpeAllowed = 10
                PrivacyEnable = 1
                SnmpMib = docsDevSwAdminStatus.0 2
            CM Config File:</td><td>tftp://192.0.2.1/boot.cm</td>
        "#;
        let flows = parse_service_flow_configs(html);
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].sf_id, Some(99369107));
        assert_eq!(flows[0].service_class, "BE");
        assert_eq!(flows[0].qos_set_type, 7);
        assert_eq!(flows[0].max_traffic_rate_bps, 86250000);
        assert_eq!(flows[0].max_concat_burst, 40000);
        assert_eq!(flows[0].admitted_qos_timeout, 200);

        let pcs = parse_packet_classifications(html);
        assert_eq!(pcs.len(), 1);
        assert_eq!(pcs[0].identifier, 48);
        assert_eq!(pcs[0].ip_protocol, 17);
        assert_eq!(pcs[0].source_port_end, Some(65535));

        let config = parse_common_config(html);
        assert_eq!(config.network_access, Some(true));
        assert_eq!(config.max_cpe_allowed, Some(10));
        assert_eq!(config.privacy_enable, Some(1));
        assert_eq!(config.upgrade_server, "172.22.48.1");
        assert_eq!(config.config_file, "tftp://192.0.2.1/boot.cm");
        assert_eq!(config.snmp_mib.len(), 1);
    }

    #[test]
    fn test_parse_product_component_and_severity_state() {
        let html = r#"
        <br><b>[3] COMMON_COMPONENTS</b>: Enabled <br>
        <table><tr><td>[ 0] CLI: Enabled </td><td>[ 1] ENVOY: Disabled </td></tr></table>
        Severity Levels
        <table><tr><td>[ 0] FATAL: Enabled </td><td>[15] INFO_VERBOSE: Disabled</td></tr></table>
        "#;
        let product = parse_product_info("", html);
        assert_eq!(product.logging_components.len(), 2);
        assert!(product.logging_components[0].enabled);
        assert!(!product.logging_components[1].enabled);
        assert_eq!(product.severity_levels.len(), 2);
        assert!(product.severity_levels[0].enabled);
        assert!(!product.severity_levels[1].enabled);
    }

    fn valid_status() -> &'static str {
        r#"
        <table>
          <tr><td>CM Status:</td><td>OPERATIONAL</td></tr>
          <tr><td>Downstream 1</td><td>3</td><td>570.00 MHz</td><td>2.20 dBmV</td><td>40.37 dB</td><td>256QAM</td><td>2858</td><td>12</td><td>0</td></tr>
        </table>
        "#
    }

    fn valid_versions() -> &'static str {
        r#"MODEL: CM3500B<br>SW_REV: TS11.05.042.02"#
    }

    #[test]
    fn parse_all_accepts_a_valid_minimal_scrape() {
        let data = parse_all(
            valid_status(),
            valid_versions(),
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            None,
            0.1,
        )
        .expect("valid status and version should parse");
        assert_eq!(data.cm_status, "OPERATIONAL");
        assert_eq!(data.version_info.model, "CM3500B");
        assert_eq!(data.downstream_qam.len(), 1);
        assert!(data.parse_status.status);
        assert!(data.parse_status.channels);
        assert!(data.parse_status.version);
        assert!(!data.parse_status.dhcp);
    }

    #[test]
    fn parse_all_rejects_empty_pages() {
        let err = parse_all("", "", "", "", "", "", "", "", "", "", None, 0.1).unwrap_err();
        assert!(err.missing.contains(&"status"));
        assert!(err.missing.contains(&"version/model"));
        assert!(!err.status.status);
        assert!(!err.status.version);
    }

    #[test]
    fn parse_all_rejects_operational_modem_without_channels() {
        let status = r#"<td>CM Status:</td><td>OPERATIONAL</td>"#;
        let err = parse_all(
            status,
            valid_versions(),
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            None,
            0.1,
        )
        .unwrap_err();
        assert_eq!(err.missing, vec!["channels"]);
        assert!(!err.status.channels);
    }

    #[test]
    fn parse_all_allows_non_operational_modem_without_channels() {
        let status = r#"<td>CM Status:</td><td>Not Ready</td>"#;
        let data = parse_all(
            status,
            valid_versions(),
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            None,
            0.1,
        )
        .expect("a non-operational modem may have no bonded channels");
        assert_eq!(data.cm_status, "Not Ready");
        assert!(data.downstream_qam.is_empty());
        assert!(data.parse_status.channels);
    }
}
