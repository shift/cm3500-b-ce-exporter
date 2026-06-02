# ARRIS CM3500B CE Prometheus Exporter

[![CI](https://github.com/shift/cm3500-b-ce-exporter/actions/workflows/ci.yml/badge.svg)](https://github.com/shift/cm3500-b-ce-exporter/actions/workflows/ci.yml)

A Prometheus exporter for the ARRIS CM3500B CE (EuroDOCSIS 3.0 / DOCSIS 3.1) cable modem. Scrapes the modem's web interface and exposes metrics in Prometheus format. Includes a Grafana dashboard and Prometheus alerting rules.

Licensed under [AGPL-3.0-or-later](LICENSE).

## Quick Start

### Nix (recommended)

```bash
nix develop --command bash -c "cargo build --release"
./target/release/cm3500-b-ce-exporter --password YOUR_PASSWORD
```

### Docker Compose (full stack)

```bash
MODEM_PASSWORD=YOUR_PASSWORD docker compose up -d
```

This starts the exporter, Prometheus, and Grafana with the dashboard pre-loaded:
- Exporter: http://localhost:10044/metrics
- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/admin)

### Binary

```bash
cm3500-b-ce-exporter --password <PASSWORD> [OPTIONS]

Options:
  --modem-url <URL>         Modem base URL [default: https://192.168.100.1]
  --username <USER>         Login username [default: admin]
  --password <PASS>         Login password (required)
  --listen <ADDR>              Listen address [default: 0.0.0.0:10044]
  --disable-prometheus         Disable the Prometheus /metrics HTTP endpoint entirely
  --interval <SECONDS>         Scrape interval [default: 30]
  --state-file <PATH>          Write connection state changes to this JSON file
  --capacity-file <PATH>       Write configured upstream/downstream capacity to this JSON file
  --state-down-threshold <N>   Consecutive bad scrapes/states before reporting down [default: 3]
  --state-up-threshold <N>     Consecutive good scrapes before reporting recovered [default: 2]
  --capacity-margin-percent <PERCENT>
                                Percent of configured service flow bandwidth to expose as shaped capacity [default: 95]
  --enable-spectrum            Actively trigger a modem spectrum scan and export spectrum metrics

  --otlp-endpoint <URL>        OTLP HTTP base URL or /v1/metrics endpoint to push metrics and logs to
  --otlp-header <KEY=VALUE>    OTLP header, can be repeated (e.g. Authorization=Basic...)
```

### OTLP Push (Grafana Cloud)

Push metrics and modem event logs directly to Grafana Cloud without running Prometheus:

```bash
cm3500-b-ce-exporter \
  --password YOUR_PASSWORD \
  --otlp-endpoint https://otlp-gateway-eu-west-0.grafana.net/otlp \
  --otlp-header "Authorization=Basic $(echo -n 'INSTANCE_ID:API_KEY' | base64)"
```

Also works with any OpenTelemetry Collector:

```bash
cm3500-b-ce-exporter \
  --password YOUR_PASSWORD \
  --otlp-endpoint http://localhost:4318/v1/metrics
```

To run in OTLP-only mode without binding any HTTP port:

```bash
cm3500-b-ce-exporter \
  --password YOUR_PASSWORD \
  --otlp-endpoint http://localhost:4318/v1/metrics \
  --disable-prometheus
```

Both Prometheus `/metrics` and OTLP push can run simultaneously. If `--otlp-endpoint` points at an OTLP base URL like `https://.../otlp`, the exporter sends metrics to `/v1/metrics` and logs to `/v1/logs` under that base. When `--disable-prometheus` is set, the exporter does not bind any HTTP port.

## Gateway Automation Outputs

The exporter can optionally write machine-readable JSON files for router/firewall automation.

### Link state file

Example:

```json
{
  "status": "up",
  "timestamp": "1748850000",
  "modem_up": true,
  "cm_status": "OPERATIONAL",
  "dhcp_state": "bound",
  "degraded": false,
  "reason": null
}
```

`status` is hysteresis-aware:
- `up` - healthy
- `degraded` - transitional or partially unhealthy
- `down` - enough consecutive bad scrapes/states to declare failure

### Capacity file

Example:

```json
{
  "timestamp": "1748850000",
  "upstream_bps": 128000000,
  "downstream_bps": 1126400000,
  "shaped_upstream_bps": 121600000,
  "shaped_downstream_bps": 1070080000,
  "source": "service_flow_config",
  "valid": true
}
```

Files are written atomically and only rewritten when their semantic content changes, making them suitable for `systemd.path` or other file-watch based automation.

Example systemd integration files are provided in [`examples/systemd/`](examples/systemd/).

## Scraping Model

The exporter scrapes the modem on a background interval (default: 30s) and serves the last successful result from `/metrics`. This avoids doing a full modem login and multi-page fetch on every Prometheus scrape and keeps scrape latency predictable despite the modem's cookie-based session handling.

## Metrics Exposed

### General

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_up` | gauge | Whether the modem scrape was successful |
| `cm3500_scrape_duration_seconds` | gauge | Duration of the last scrape |
| `cm3500_info` | gauge | Modem hardware/firmware info (labels: model, serial, firmware, etc.) |
| `cm3500_uptime_seconds` | gauge | System uptime in seconds |
| `cm3500_cm_status` | gauge | Operational status label |
| `cm3500_interface_up` | gauge | Interface operational status (1 = Up) |
| `cm3500_cpe_static` | gauge | Number of static CPE devices detected |
| `cm3500_cpe_dynamic` | gauge | Number of dynamic CPE devices detected |

### DHCP

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_dhcp_state` | gauge | DHCP state (bound, etc.) |
| `cm3500_dhcp_lease_total_seconds` | gauge | DHCP lease duration |
| `cm3500_dhcp_lease_remaining_seconds` | gauge | DHCP lease time remaining |
| `cm3500_dhcp_rebind_remaining_seconds` | gauge | DHCP rebind time remaining |
| `cm3500_dhcp_renew_remaining_seconds` | gauge | DHCP renew time remaining |

### Downstream SC-QAM

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `cm3500_downstream_qam_power_dbmv` | gauge | channel, dcid, frequency_mhz, modulation | Power level in dBmV |
| `cm3500_downstream_qam_snr_db` | gauge | channel, dcid, frequency_mhz, modulation | Signal-to-noise ratio in dB |
| `cm3500_downstream_qam_octets_total` | counter | channel, dcid | Total octets received |
| `cm3500_downstream_qam_corrected_total` | counter | channel, dcid | Corrected FEC errors |
| `cm3500_downstream_qam_uncorrectable_total` | counter | channel, dcid | Uncorrectable FEC errors |

### Downstream OFDM

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `cm3500_downstream_ofdm_channel_width_mhz` | gauge | channel, fft_type | Channel width in MHz |
| `cm3500_downstream_ofdm_active_subcarriers` | gauge | channel | Number of active subcarriers |
| `cm3500_downstream_ofdm_rxmer_db` | gauge | channel, measurement (pilot/plc/data) | Receive MER in dB |

### Upstream SC-QAM

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `cm3500_upstream_qam_power_dbmv` | gauge | channel, ucid, frequency_mhz, modulation, channel_type | Transmit power in dBmV |
| `cm3500_upstream_qam_symbol_rate_ksym` | gauge | channel, ucid | Symbol rate in kSym/s |

### Upstream OFDMA

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `cm3500_upstream_ofdma_tx_power_dbmv` | gauge | channel, fft_type | Transmit power in dBmV |
| `cm3500_upstream_ofdma_channel_width_mhz` | gauge | channel | Channel width in MHz |
| `cm3500_upstream_ofdma_active_subcarriers` | gauge | channel | Number of active subcarriers |
| `cm3500_upstream_ofdma_active_subcarrier_range_mhz` | gauge | channel, edge | First/last active subcarrier positions |

### QoS & DOCSIS State

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_qos_packets_total` | counter | Total packets per QoS service flow (labels: sfid, direction, primary, service_class) |
| `cm3500_docsis_state` | gauge | Overall DOCSIS registration state |
| `cm3500_docsis_phase` | gauge | Per-phase registration status (labels: phase, status) |
| `cm3500_bpi_state` | gauge | Baseline Privacy Interface state |
| `cm3500_tod_state` | gauge | Time of Day acquisition state |

### Event Log

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_event_log_entries` | gauge | Entry count in the event log by event ID |
| `cm3500_event_t3_timeouts` | gauge | T3 timeouts (No Ranging Response) in event log |
| `cm3500_event_t4_timeouts` | gauge | T4 timeouts in event log |
| `cm3500_event_dhcp_failures` | gauge | DHCP failures in event log |
| `cm3500_event_dhcp_renew_warnings` | gauge | DHCP renew warnings in event log |
| `cm3500_event_profile_changes` | gauge | Upstream profile assignment changes |
| `cm3500_event_dhcp_renew_no_response` | gauge | DHCP renew sent with no response events |
| `cm3500_event_upstream_active_profile` | gauge | Last seen upstream profile ID from event log |
| `cm3500_event_ofdma_profile_id` | gauge | Last seen OFDM/OFDMA profile ID from event log |

### Product & Spectrum

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_product_ethernet_phy_info` | gauge | Ethernet PHY capability label from the modem |
| `cm3500_product_logging_components_enabled` | gauge | Enabled internal logging components by debug group |
| `cm3500_spectrum_chunks` | gauge | Number of spectrum chunks returned by a scan |
| `cm3500_spectrum_chunk_power_dbmv` | gauge | Per-chunk min/avg/max power summary |
| `cm3500_spectrum_chunk_metadata` | gauge | Per-chunk span/bin spacing/RBW metadata |
| `cm3500_spectrum_bin_power_dbmv` | gauge | Per-bin spectrum power from the modem scan |

### QoS Configuration (from config_params_cgi)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `cm3500_qos_max_traffic_rate_kbps` | gauge | direction, flow_index | Maximum traffic rate from config |
| `cm3500_qos_min_reserved_rate_kbps` | gauge | direction, flow_index | Minimum reserved rate from config |
| `cm3500_qos_max_traffic_burst` | gauge | direction, flow_index | Maximum traffic burst from config |
| `cm3500_qos_traffic_priority` | gauge | direction, flow_index | Traffic priority from config |

### Key Event IDs

| Event ID | Description |
|----------|-------------|
| `82000200` | T3 timeout — No Ranging Response received |
| `82000300` | T4 timeout |
| `68001202` | DHCP failed — Solicit sent, no Advertise received |
| `68010100` | DHCP RENEW sent — No response for IPv4 |
| `68010300` | DHCP RENEW WARNING — Field invalid in response |
| `68010700` | Primary lease failed, IPv4 fallback initiated |
| `67061601` | US profile assignment change |
| `74010100` | CM-STATUS message (OFDM/OFDMA profile change) |
| `75012500` | US Diplexer Mode Initialization |
| `75012600` | DS Diplexer Mode Initialization |
| `69010200` | Software download initiated |
| `69011200` | Software download successful |
| `90000000` | MIMO event |

## Alerting Rules

Rules are split into three files. Always load `alert_rules_common.yml`, then choose **one** plant-specific file:

- `alert_rules_eurodocsis.yml` — Vodafone DE, Liberty Global, Virgin Media O2, etc.
- `alert_rules_docsis_us.yml` — Comcast, Spectrum, Cox, etc.

### Common Rules (`alert_rules_common.yml`)

Apply to all deployments regardless of plant type:

| Alert | Severity | Condition |
|-------|----------|----------|
| `CM3500ModemDown` | critical | Scrape failing for 2m |
| `CM3500ScrapeSlow` | warning | Scrape taking >5s for 5m |
| `CM3500Rebooted` | info | Uptime decreased |
| `CM3500NotOperational` | critical | Status != OPERATIONAL |
| `CM3500InterfaceDown` | critical | Interface not Up for 2m |
| `CM3500DownstreamSNRLow` | warning | SNR < 33 dB for 5m |
| `CM3500DownstreamSNRCritical` | critical | SNR < 23 dB (64QAM) or < 30 dB (256QAM) |
| `CM3500UncorrectableErrors` | warning | Uncorrectable FEC errors increasing |
| `CM3500T3Timeouts` | warning | T3 timeouts increasing over 10m |
| `CM3500T4Timeouts` | warning | T4 timeouts increasing over 10m |
| `CM3500DHCPFailures` | warning | DHCP failures increasing over 10m |
| `CM3500DHCPRenewWarnings` | warning | DHCP renew warnings > 5 in 10m |
| `CM3500ProfileChanges` | info | US profile changes increasing over 10m |
| `CM3500DHCPLeaseExpiring` | warning | Lease remaining < 60s for 2m |
| `CM3500OFDMRxMERLow` | warning | OFDM data RxMER < 31 dB |
| `CM3500OFDMRxMERCritical` | critical | OFDM data RxMER < 25 dB |

### EuroDOCSIS Rules (`alert_rules_eurodocsis.yml`)

For European cable plants. Key thresholds:
- Downstream low power: **-5 dBmV** (European outlet spec 56–73 dBuV; plant tilt makes lower channels vulnerable)
- Upstream warning: **49 dBmV** (Vodafone DE plants degrade past 50 dBmV on bonded channels)
- Upstream critical: **53 dBmV** (EuroDOCSIS bonded-channel maximum)

| Alert | Severity | Condition |
|-------|----------|----------|
| `CM3500EuroDownstreamPowerLow` | warning | DS power < -5 dBmV for 5m |
| `CM3500EuroDownstreamPowerHigh` | warning | DS power > +12 dBmV for 5m |
| `CM3500EuroUpstreamPowerHigh` | warning | US power > 49 dBmV for 5m |
| `CM3500EuroUpstreamPowerCritical` | critical | US power > 53 dBmV for 5m |

### US DOCSIS Rules (`alert_rules_docsis_us.yml`)

For North American cable plants. Key thresholds:
- Downstream low power: **-7 dBmV** (standard ISP operating window -7 to +7 dBmV)
- Upstream warning: **51 dBmV** (bonded 4ch ATDMA/64QAM maximum)
- Upstream critical: **55 dBmV** (well above bonded-channel limit)

| Alert | Severity | Condition |
|-------|----------|----------|
| `CM3500USDownstreamPowerLow` | warning | DS power < -7 dBmV for 5m |
| `CM3500USDownstreamPowerHigh` | warning | DS power > +12 dBmV for 5m |
| `CM3500USUpstreamPowerHigh` | warning | US power > 51 dBmV for 5m |
| `CM3500USUpstreamPowerCritical` | critical | US power > 55 dBmV for 5m |

The dashboard (`grafana/dashboard.json`) contains the following sections:

- **Overview** — Status, uptime, DHCP lease gauge, BPI state, CPE count, modem info table
- **Downstream SC-QAM** — Power level timeseries with DOCSIS thresholds, SNR timeseries, corrected/uncorrectable error bars, power bar chart
- **Downstream OFDM** — RxMER timeseries (pilot/plc/data), channel details table
- **Upstream** — Power level timeseries (QAM + OFDMA) with thresholds, symbol rate
- **DHCP & Lease** — Lease/renew/rebind timers over time
- **Event Log** — Critical event stat panels, event count bar chart by ID, event trends
- **DOCSIS State** — Registration phases table, QoS service flow table

## Prometheus Configuration

```yaml
scrape_configs:
  - job_name: 'cm3500'
    static_configs:
      - targets: ['localhost:10044']
    scrape_interval: 30s

rule_files:
  - "alert_rules_common.yml"
  - "alert_rules_eurodocsis.yml"  # or alert_rules_docsis_us.yml
```

## Testing

The test suite includes fixture-based OTLP snapshot tests for both metrics and modem event logs.

Snapshot files:
- `tests/fixtures/otlp_metrics_snapshot.txt`
- `tests/fixtures/otlp_logs_snapshot.txt`

If OTLP output changes intentionally, run the test suite, inspect the assertion diff, update the snapshot files, and rerun tests.

## Project Structure

```
cm3500-b-ce-exporter/
├── src/
│   ├── main.rs              # CLI, HTTP server, background scraper/cache
│   ├── client.rs            # Modem HTTP client (cookie auth, auto re-login)
│   ├── parser.rs            # HTML parsing for all endpoints
│   ├── metrics.rs           # Prometheus text format rendering
│   └── otlp.rs              # OTLP/HTTP JSON push for metrics and event logs
├── grafana/
│   ├── dashboard.json       # Pre-built Grafana dashboard
│   └── provisioning/        # Auto-provisioning configs
│       ├── dashboards/
│       └── datasources/
├── prometheus/
│   ├── prometheus.yml               # Prometheus config
│   ├── alert_rules_common.yml      # Common alerts (all regions)
│   ├── alert_rules_eurodocsis.yml   # EuroDOCSIS power thresholds
│   └── alert_rules_docsis_us.yml   # US DOCSIS power thresholds
├── flake.nix                # Nix dev shell
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
└── README.md
```
