use crate::parser::ScrapedData;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ScrapeObservation {
    modem_up: bool,
    cm_status: String,
    dhcp_state: String,
    reason: Option<String>,
    capacity: CapacitySemantic,
}

impl ScrapeObservation {
    pub fn from_data(data: &ScrapedData, capacity_margin_percent: u8) -> Self {
        Self {
            modem_up: true,
            cm_status: data.cm_status.clone(),
            dhcp_state: data.dhcp_state.clone(),
            reason: non_operational_reason(&data.cm_status, &data.dhcp_state),
            capacity: CapacitySemantic::from_data(data, capacity_margin_percent),
        }
    }

    pub fn from_error(reason: String, capacity_margin_percent: u8) -> Self {
        Self {
            modem_up: false,
            cm_status: String::new(),
            dhcp_state: String::new(),
            reason: Some(reason),
            capacity: CapacitySemantic::invalid(capacity_margin_percent),
        }
    }

    fn is_good(&self) -> bool {
        self.modem_up
            && self.cm_status.eq_ignore_ascii_case("OPERATIONAL")
            && (self.dhcp_state.is_empty() || self.dhcp_state.eq_ignore_ascii_case("bound"))
    }
}

pub struct AutomationOutputs {
    state_file: Option<PathBuf>,
    capacity_file: Option<PathBuf>,
    tracker: LinkStateTracker,
    last_state: Option<LinkStateSemantic>,
    last_capacity: Option<CapacitySemantic>,
}

impl AutomationOutputs {
    pub fn new(
        state_file: Option<PathBuf>,
        capacity_file: Option<PathBuf>,
        state_down_threshold: u32,
        state_up_threshold: u32,
    ) -> Option<Self> {
        if state_file.is_none() && capacity_file.is_none() {
            return None;
        }

        Some(Self {
            state_file,
            capacity_file,
            tracker: LinkStateTracker::new(state_down_threshold, state_up_threshold),
            last_state: None,
            last_capacity: None,
        })
    }

    pub fn update(&mut self, observation: &ScrapeObservation) -> Result<()> {
        if let Some(path) = &self.state_file {
            let semantic = self.tracker.observe(observation);
            if self.last_state.as_ref() != Some(&semantic) {
                write_json_atomic(path, &LinkStateFile::from_semantic(&semantic))?;
                self.last_state = Some(semantic);
            }
        }

        if let Some(path) = &self.capacity_file {
            let capacity = observation.capacity.clone();
            if self.last_capacity.as_ref() != Some(&capacity) {
                write_json_atomic(path, &CapacityFile::from_semantic(&capacity))?;
                self.last_capacity = Some(capacity);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkStateSemantic {
    status: &'static str,
    modem_up: bool,
    cm_status: String,
    dhcp_state: String,
    degraded: bool,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct LinkStateFile {
    status: &'static str,
    timestamp: String,
    modem_up: bool,
    cm_status: String,
    dhcp_state: String,
    degraded: bool,
    reason: Option<String>,
}

impl LinkStateFile {
    fn from_semantic(semantic: &LinkStateSemantic) -> Self {
        Self {
            status: semantic.status,
            timestamp: iso_timestamp_now(),
            modem_up: semantic.modem_up,
            cm_status: semantic.cm_status.clone(),
            dhcp_state: semantic.dhcp_state.clone(),
            degraded: semantic.degraded,
            reason: semantic.reason.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapacitySemantic {
    upstream_bps: Option<u64>,
    downstream_bps: Option<u64>,
    shaped_upstream_bps: Option<u64>,
    shaped_downstream_bps: Option<u64>,
    source: &'static str,
    valid: bool,
}

impl CapacitySemantic {
    fn from_data(data: &ScrapedData, margin_percent: u8) -> Self {
        let upstream_bps = max_rate_bps(&data.service_flow_configs, "Upstream");
        let downstream_bps = max_rate_bps(&data.service_flow_configs, "Downstream");
        Self {
            shaped_upstream_bps: upstream_bps.map(|v| apply_margin(v, margin_percent)),
            shaped_downstream_bps: downstream_bps.map(|v| apply_margin(v, margin_percent)),
            upstream_bps,
            downstream_bps,
            source: "service_flow_config",
            valid: upstream_bps.is_some() || downstream_bps.is_some(),
        }
    }

    fn invalid(_margin_percent: u8) -> Self {
        Self {
            upstream_bps: None,
            downstream_bps: None,
            shaped_upstream_bps: None,
            shaped_downstream_bps: None,
            source: "service_flow_config",
            valid: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct CapacityFile {
    timestamp: String,
    upstream_bps: Option<u64>,
    downstream_bps: Option<u64>,
    shaped_upstream_bps: Option<u64>,
    shaped_downstream_bps: Option<u64>,
    source: &'static str,
    valid: bool,
}

impl CapacityFile {
    fn from_semantic(semantic: &CapacitySemantic) -> Self {
        Self {
            timestamp: iso_timestamp_now(),
            upstream_bps: semantic.upstream_bps,
            downstream_bps: semantic.downstream_bps,
            shaped_upstream_bps: semantic.shaped_upstream_bps,
            shaped_downstream_bps: semantic.shaped_downstream_bps,
            source: semantic.source,
            valid: semantic.valid,
        }
    }
}

struct LinkStateTracker {
    stable_up: Option<bool>,
    consecutive_good: u32,
    consecutive_bad: u32,
    down_threshold: u32,
    up_threshold: u32,
}

impl LinkStateTracker {
    fn new(down_threshold: u32, up_threshold: u32) -> Self {
        Self {
            stable_up: None,
            consecutive_good: 0,
            consecutive_bad: 0,
            down_threshold: down_threshold.max(1),
            up_threshold: up_threshold.max(1),
        }
    }

    fn observe(&mut self, observation: &ScrapeObservation) -> LinkStateSemantic {
        let good = observation.is_good();
        match self.stable_up {
            None => {
                self.stable_up = Some(good);
                self.consecutive_good = u32::from(good);
                self.consecutive_bad = u32::from(!good);
            }
            Some(stable_up) if good == stable_up => {
                if good {
                    self.consecutive_good += 1;
                    self.consecutive_bad = 0;
                } else {
                    self.consecutive_bad += 1;
                    self.consecutive_good = 0;
                }
            }
            Some(true) => {
                self.consecutive_bad += 1;
                self.consecutive_good = 0;
                if self.consecutive_bad >= self.down_threshold {
                    self.stable_up = Some(false);
                }
            }
            Some(false) => {
                self.consecutive_good += 1;
                self.consecutive_bad = 0;
                if self.consecutive_good >= self.up_threshold {
                    self.stable_up = Some(true);
                }
            }
        }

        let stable_up = self.stable_up.unwrap_or(false);
        let degraded = good != stable_up;
        let status = match (stable_up, degraded) {
            (true, false) => "up",
            (false, false) => "down",
            _ => "degraded",
        };

        LinkStateSemantic {
            status,
            modem_up: observation.modem_up,
            cm_status: observation.cm_status.clone(),
            dhcp_state: observation.dhcp_state.clone(),
            degraded,
            reason: observation.reason.clone(),
        }
    }
}

fn non_operational_reason(cm_status: &str, dhcp_state: &str) -> Option<String> {
    if !cm_status.eq_ignore_ascii_case("OPERATIONAL") {
        return Some(if cm_status.is_empty() {
            "cm_status_unknown".to_string()
        } else {
            format!("cm_status_{}", normalize_token(cm_status))
        });
    }

    if !dhcp_state.is_empty() && !dhcp_state.eq_ignore_ascii_case("bound") {
        return Some(format!("dhcp_state_{}", normalize_token(dhcp_state)));
    }

    None
}

fn normalize_token(s: &str) -> String {
    s.to_lowercase().replace([' ', '-', '/'], "_")
}

fn max_rate_bps(flows: &[crate::parser::ServiceFlowConfig], direction: &str) -> Option<u64> {
    flows
        .iter()
        .filter(|f| f.direction.eq_ignore_ascii_case(direction))
        .map(|f| f.max_traffic_rate_bps)
        .max()
}

fn apply_margin(value: u64, margin_percent: u8) -> u64 {
    value.saturating_mul(margin_percent as u64) / 100
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    let content = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp_path, content)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn iso_timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{
        CmState, DhcpInfo, ProductInfo, ScrapedData, ServiceFlowConfig, SpectrumChunk, VersionInfo,
    };

    #[test]
    fn hysteresis_transitions_to_degraded_before_down() {
        let mut tracker = LinkStateTracker::new(3, 2);
        let good = ScrapeObservation {
            modem_up: true,
            cm_status: "OPERATIONAL".into(),
            dhcp_state: "bound".into(),
            reason: None,
            capacity: CapacitySemantic::invalid(100),
        };
        let bad = ScrapeObservation::from_error("scrape_failed".into(), 100);

        assert_eq!(tracker.observe(&good).status, "up");
        assert_eq!(tracker.observe(&bad).status, "degraded");
        assert_eq!(tracker.observe(&bad).status, "degraded");
        assert_eq!(tracker.observe(&bad).status, "down");
    }

    #[test]
    fn capacity_uses_max_service_flow_rate_with_margin() {
        let data = ScrapedData {
            downstream_qam: vec![],
            downstream_ofdm: vec![],
            upstream_qam: vec![],
            upstream_ofdm: vec![],
            uptime_seconds: None,
            cm_status: "OPERATIONAL".into(),
            version_info: VersionInfo {
                hardware_rev: String::new(),
                vendor: String::new(),
                bootloader: String::new(),
                software_rev: String::new(),
                model: String::new(),
                serial: String::new(),
                firmware_name: String::new(),
                firmware_build_time: String::new(),
            },
            dhcp_info: DhcpInfo {
                cm_ip: String::new(),
                cm_subnet: String::new(),
                cm_gateway: String::new(),
                lease_total_secs: None,
                lease_remaining_secs: None,
                rebind_total_secs: None,
                rebind_remaining_secs: None,
                renew_total_secs: None,
                renew_remaining_secs: None,
            },
            qos_flows: vec![],
            cm_state: CmState {
                overall_state: String::new(),
                phases: vec![],
                bpi_status: String::new(),
                tod_status: String::new(),
            },
            events: vec![],
            interfaces: vec![],
            product_info: ProductInfo {
                ethernet_phy_type: String::new(),
                logging_components_enabled: vec![],
            },
            spectrum: Vec::<SpectrumChunk>::new(),
            cpe_static: 0,
            cpe_dynamic: 0,
            dhcp_state: "bound".into(),
            service_flow_configs: vec![
                ServiceFlowConfig {
                    direction: "Upstream".into(),
                    index: 0,
                    max_traffic_rate_bps: 128000,
                    max_traffic_burst: 0,
                    min_reserved_rate_bps: 0,
                    traffic_priority: 0,
                    scheduling_type: 0,
                },
                ServiceFlowConfig {
                    direction: "Downstream".into(),
                    index: 1,
                    max_traffic_rate_bps: 128000,
                    max_traffic_burst: 0,
                    min_reserved_rate_bps: 0,
                    traffic_priority: 0,
                    scheduling_type: 0,
                },
                ServiceFlowConfig {
                    direction: "Downstream".into(),
                    index: 2,
                    max_traffic_rate_bps: 1126400,
                    max_traffic_burst: 0,
                    min_reserved_rate_bps: 0,
                    traffic_priority: 0,
                    scheduling_type: 0,
                },
            ],
            scrape_duration_secs: 0.0,
        };

        let capacity = CapacitySemantic::from_data(&data, 95);
        assert_eq!(capacity.upstream_bps, Some(128_000));
        assert_eq!(capacity.downstream_bps, Some(1_126_400));
        assert_eq!(capacity.shaped_upstream_bps, Some(121_600));
        assert_eq!(capacity.shaped_downstream_bps, Some(1_070_080));
    }
}
