# ARRIS CM3500B CE Prometheus Exporter

[![CI](https://github.com/shift/cm3500-b-ce-exporter/actions/workflows/ci.yml/badge.svg)](https://github.com/shift/cm3500-b-ce-exporter/actions/workflows/ci.yml)

A Prometheus exporter for the ARRIS CM3500B CE (EuroDOCSIS 3.0 / DOCSIS 3.1) cable modem. Scrapes the modem's web interface and exposes metrics in Prometheus format. Includes a Grafana dashboard and Prometheus alerting rules.

Licensed under [AGPL-3.0-or-later](LICENSE). See [CHANGELOG.md](CHANGELOG.md)
for release and upgrade notes.

## Quick Start

### Nix (recommended)

```bash
nix develop --command bash -c "cargo build --release"
./target/release/cm3500-b-ce-exporter --password YOUR_PASSWORD --insecure-tls
```

The CM3500B uses a fixed ARRIS device certificate and does not provide
certificate management. Its name does not match the modem's IP address, so
ordinary CA validation cannot succeed. You have two options:

- `--insecure-tls` accepts the certificate without authentication.
- `--tls-cert-fingerprint` pins the leaf certificate's SHA-256 fingerprint. This
  keeps authentication while tolerating the certificate's name and unknown
  issuer, and is the preferred option.

Get the fingerprint directly from the modem:

```bash
openssl s_client -connect 192.168.100.1:443 -servername 192.168.100.1 </dev/null 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha256
```

Then run with:

```bash
./target/release/cm3500-b-ce-exporter \
  --password YOUR_PASSWORD \
  --tls-cert-fingerprint "SHA256:25:05:2E:...:11:79"
```

The certificate is valid for twenty years, so the fingerprint is stable. If a
firmware update replaces it, the exporter will fail TLS verification until the
fingerprint is updated. That failure is intentional.

For Compose, set `MODEM_TLS_FINGERPRINT` in `.env`. When it is empty the
exporter falls back to `--insecure-tls`; when set, the fingerprint takes
precedence and the insecure flag is ignored with a warning.

### Docker Compose (full stack)

```bash
cp .env.example .env
# edit .env and set MODEM_PASSWORD and GRAFANA_ADMIN_PASSWORD
docker compose up -d
```

This starts the exporter, Prometheus, and Grafana with the dashboard pre-loaded:
- Exporter metrics are internal to the Compose network (not published)
- Prometheus: http://127.0.0.1:9090
- Alertmanager: http://127.0.0.1:9093
- Grafana: http://127.0.0.1:3000

Alertmanager ships with an empty default receiver so alerts are visible in its
UI but not delivered. Add your email/webhook/PagerDuty receiver in
`prometheus/alertmanager.yml` to page.

The exporter endpoint contains modem serial numbers, interface MAC addresses,
and network state. Treat `/metrics` as sensitive internal telemetry and do not
publish it beyond a trusted monitoring network.

### Binary

```bash
cm3500-b-ce-exporter --password <PASSWORD> [OPTIONS]

Options:
  --modem-url <URL>         Modem base URL [default: https://192.168.100.1]
  --username <USER>         Login username [default: admin]
  --password <PASS>         Login password (or set MODEM_PASSWORD)
  --listen <ADDR>              Listen address [default: 127.0.0.1:10044]
  --disable-prometheus         Disable the Prometheus /metrics HTTP endpoint entirely
  --interval <SECONDS>         Scrape interval [default: 30]
  --connect-timeout <SECONDS>  TCP connection timeout [default: 5]
  --request-timeout <SECONDS>  Total HTTP request timeout [default: 15]
  --insecure-tls               Accept the modem's self-signed/invalid TLS certificate
  --ca-cert <PATH>             PEM CA certificate to trust instead of --insecure-tls
  --tls-cert-fingerprint <SHA256>
                                Pin the modem's leaf certificate by SHA-256 fingerprint
  --debug-html-dir <DIR>       Write redacted modem HTML on structural parse failure
  --readiness-stale-after <SECONDS>
                                Fail /ready after this long without a successful scrape
                                [default: max(60, 3 × interval)]
  --state-file <PATH>          Write connection state changes to this JSON file
  --capacity-file <PATH>       Write configured upstream/downstream capacity to this JSON file
  --state-down-threshold <N>   Consecutive bad scrapes/states before reporting down [default: 3]
  --state-up-threshold <N>     Consecutive good scrapes before reporting recovered [default: 2]
  --capacity-margin-percent <PERCENT>
                                Percent of configured service flow bandwidth to expose as shaped capacity [default: 95]
  --enable-spectrum            Actively trigger a modem spectrum scan and export spectrum metrics
  --spectrum-interval <SECONDS>
                                Interval between active spectrum scans [default: 900]

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

OTLP backends that translate metric names may append type-related suffixes
(for example `_total` or `_bytes`). If you use the supplied OTLP alert rules or
dashboard, configure the collector with `add_metric_suffixes=false`, or update
the PromQL names to match your pipeline.

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
  "upstream_bps": 86250000,
  "downstream_bps": 690000000,
  "shaped_upstream_bps": 81937500,
  "shaped_downstream_bps": 655500000,
  "source": "service_flow_config",
  "basis": "max_individual_service_flow_rate",
  "margin_percent": 95,
  "valid": true
}
```

Capacity semantics:

- `upstream_bps` / `downstream_bps` are the highest individual provisioned
  `SfMaxTrafficRate` across the service flows in that direction. The modem
  reports this value in bits per second. These are not an aggregate across
  flows and are not measured throughput.
- `shaped_*_bps` apply `--capacity-margin-percent` to those provisioned rates.
  They are an operational estimate of usable capacity, not a measurement.
- DOCSIS service-flow configurations can contain multiple flows per direction;
  the current basis deliberately takes the largest individual flow. Verify that
  assumption against your provider's configuration before using these values for
  shaping decisions.

OTLP exposes the same values as `hw.cable_modem.capacity.provisioned` and
`hw.cable_modem.capacity.estimated`.

Files are written atomically and only rewritten when their semantic content changes, making them suitable for `systemd.path` or other file-watch based automation.

Example systemd integration files are provided in [`examples/systemd/`](examples/systemd/).

## Spectrum Scanning

When `--enable-spectrum` is set, the exporter performs an active modem spectrum scan and exposes the results as Prometheus and OTLP metrics. Spectrum scans use their own interval via `--spectrum-interval` and the most recent successful scan result is cached between normal scrape cycles.

## Scraping Model

The exporter scrapes the modem on a background interval (default: 30s) and serves the last scrape result from `/metrics`. This avoids doing a full modem login and multi-page fetch on every Prometheus scrape and keeps scrape latency predictable despite the modem's cookie-based session handling.

Every modem page linked from the web interface is fetched and parsed:

`status_cgi`, `vers_cgi`, `dhcp_cgi`, `qos_cgi`, `cm_state_cgi`, `event_cgi`,
`config_params_cgi`, `product_cgi`, `dhcp_cmLog_cgi`, and `dsx_cgi`. Spectrum
data is fetched only when `--enable-spectrum` is set, using `spectrum_cgi`.
Static Open Source attribution content is not telemetry and is not scraped.

A scrape is only considered successful if the required page fields parse:

- the status page yields a CM status and at least one downstream channel when the
  modem reports `OPERATIONAL`;
- the version page yields a model.

If an expected field is missing, `/metrics` reports `cm3500_up 0`,
`cm3500_parse_success 0`, and the relevant
`cm3500_section_parse_success{section="..."}` indicator. This prevents a
firmware change or truncated page from looking like a healthy scrape with empty
channel metrics.

## Health and Readiness

- `/health` is a liveness check and returns `200 OK` whenever the process is
  running.
- `/ready` returns `200` only if a scrape has completed successfully within the
  staleness window (`--readiness-stale-after`, or `max(60, 3 × interval)` by
  default). Otherwise it returns `503` with the age of the last success and the
  most recent error.
- `/metrics` always serves the most recent result, successful or not.

Use `/ready` for container orchestration and external alerting; use `/health`
only to detect a dead process. An initial login failure is logged but does not
exit the process, so the HTTP endpoints remain available while the scrape loop
retries the modem.

## Debugging Failed Parses

Set `--debug-html-dir /var/lib/cm3500/debug` to preserve the modem responses
when an essential field fails to parse. Each failure creates a timestamped
directory containing the raw pages and `parse-error.txt`.

Debug bundles use `0700` directories and `0600` files. Token-like values
(`csrf_token`, passwords, session/token fields) are redacted, but the pages
still contain modem serial numbers, MAC addresses, and network details. Protect
and rotate these files accordingly.

## Metrics Exposed

### General

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_up` | gauge | Whether the complete fetch-and-parse scrape was successful |
| `cm3500_parse_success` | gauge | Whether the required modem page fields parsed |
| `cm3500_section_parse_success` | gauge | Per-section parse status (label: `section`) |
| `cm3500_scrape_duration_seconds` | gauge | Duration of the last scrape |
| `cm3500_info` | gauge | Modem hardware/firmware info, including firmware build time (labels: model, serial, firmware, etc.) |
| `cm3500_uptime_seconds` | gauge | System uptime in seconds |
| `cm3500_cm_status` | gauge | Operational status label |
| `cm3500_interface_up` | gauge | Interface operational status (1 = Up) |
| `cm3500_cpe_static` | gauge | Number of static CPE devices detected |
| `cm3500_cpe_dynamic` | gauge | Number of dynamic CPE devices detected |
| `cm3500_status_info` | gauge | Product type, hardware info, router/telemetry state, RF hardware, market |
| `cm3500_l2vpn_attachment_circuit_info` | gauge | L2VPN attachment circuit IDs (labels: index, id) |
| `cm3500_diplexer_band_info` | gauge | Diplexer band ranges and current band setting |
| `cm3500_config_file_info` | gauge | Configuration file URL |
| `cm3500_config_upgrade_server_info` | gauge | Configured upgrade server |
| `cm3500_config_network_access` | gauge | Whether network access is enabled |
| `cm3500_config_max_cpe_allowed` | gauge | Maximum allowed CPE count |
| `cm3500_config_privacy_enable` | gauge | Privacy enable setting |
| `cm3500_config_snmp_mib_info` | gauge | SNMP MIB settings |

### DHCP

| Metric | Type | Description |
|--------|------|-------------|
| `cm3500_dhcp_state` | gauge | DHCP state |
| `cm3500_dhcp_lease_total_seconds` | gauge | DHCP lease duration |
| `cm3500_dhcp_lease_remaining_seconds` | gauge | DHCP lease time remaining |
| `cm3500_dhcp_rebind_total_seconds` | gauge | Total DHCP rebind duration |
| `cm3500_dhcp_rebind_remaining_seconds` | gauge | DHCP rebind time remaining |
| `cm3500_dhcp_renew_total_seconds` | gauge | Total DHCP renew duration |
| `cm3500_dhcp_renew_remaining_seconds` | gauge | DHCP renew time remaining |
| `cm3500_dhcp_cm_ip_info` | gauge | Assigned CM IP address |
| `cm3500_dhcp_cm_subnet_info` | gauge | CM subnet mask |
| `cm3500_dhcp_cm_gateway_info` | gauge | CM gateway |
| `cm3500_dhcp_tftp_server_info` | gauge | TFTP server |
| `cm3500_dhcp_time_server_info` | gauge | Time server |
| `cm3500_dhcp_boot_file_info` | gauge | Boot file |
| `cm3500_dhcp_software_filename_info` | gauge | Software filename |
| `cm3500_dhcp_time_offset_seconds` | gauge | DHCP time offset |
| `cm3500_dhcp_mdd_ip_override` | gauge | MDD IP override value |
| `cm3500_dhcp_mdd_ip_override_enabled` | gauge | Whether MDD IP override is enabled |
| `cm3500_dhcp_mdd_ip_mode` | gauge | MDD IP mode |
| `cm3500_dhcp_attempts` | gauge | DHCP attempts by family (label: family) |
| `cm3500_dhcp_log_entries` | gauge | Number of DHCPv6 log entries |
| `cm3500_dhcp_log_message_type_entries` | gauge | DHCPv6 log entries by message type |
| `cm3500_dhcp_log_entry_info` | gauge | DHCPv6 log entry details |

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
| `cm3500_downstream_ofdm_active_subcarrier_range_mhz` | gauge | channel, edge | First/last active subcarrier positions |
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
| `cm3500_upstream_ofdma_unlabeled_value` | gauge | channel, column_index | Row values present in the modem table but not named by its header |

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
| `cm3500_product_logging_component` | gauge | Per-component logging state (1 = enabled) |
| `cm3500_product_severity_level` | gauge | Per-severity logging state (1 = enabled) |
| `cm3500_dsx_log_entries` | gauge | Number of DSX message log entries |
| `cm3500_dsx_log_entry_info` | gauge | DSX log entry details |
| `cm3500_spectrum_scan_age_seconds` | gauge | Age of the cached spectrum scan |
| `cm3500_spectrum_chunks` | gauge | Number of spectrum chunks returned by a scan |
| `cm3500_spectrum_chunk_power_dbmv` | gauge | Per-chunk min/avg/max power summary |
| `cm3500_spectrum_chunk_metadata` | gauge | Per-chunk span/bin spacing/RBW metadata |
| `cm3500_spectrum_bin_power_dbmv` | gauge | Per-bin spectrum power from the modem scan |

### QoS Configuration (from config_params_cgi)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `cm3500_qos_max_traffic_rate_bps` | gauge | direction, flow_index | Maximum traffic rate from config (bits/s) |
| `cm3500_qos_min_reserved_rate_bps` | gauge | direction, flow_index | Minimum reserved rate from config (bits/s) |
| `cm3500_qos_max_traffic_burst` | gauge | direction, flow_index | Maximum traffic burst from config |
| `cm3500_qos_traffic_priority` | gauge | direction, flow_index | Traffic priority from config |
| `cm3500_qos_service_flow_info` | gauge | direction, flow_index, reference, sf_id, service_class | Service-flow identity and class |
| `cm3500_qos_set_type` | gauge | direction, flow_index | QoS parameter set type |
| `cm3500_qos_min_reserved_rate_pktsize` | gauge | direction, flow_index | Minimum reserved rate packet size |
| `cm3500_qos_active_qos_timeout` | gauge | direction, flow_index | Active QoS timeout |
| `cm3500_qos_admitted_qos_timeout` | gauge | direction, flow_index | Admitted QoS timeout |
| `cm3500_qos_max_concat_burst` | gauge | direction, flow_index | Maximum concatenated burst |
| `cm3500_qos_packet_classification_info` | gauge | direction, index, service_flow, identifier, rule_priority, activation_state, ip_protocol, port ranges | Packet classification rules |
| `cm3500_capacity_provisioned_bps` | gauge | direction, basis | Highest individual provisioned flow rate (not aggregate or measured) |
| `cm3500_capacity_estimated_bps` | gauge | direction, basis, margin_percent | Provisioned rate multiplied by the configured margin (an estimate) |

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

Two sets of alert rules and dashboards are provided:

| Set | Dashboard | Alert rules | Use when |
|-----|-----------|-------------|----------|
| **Prometheus** | `grafana/dashboard.json` | `alert_rules_common.yml` + plant-specific | Scraping `/metrics` directly with Prometheus |
| **OTLP** | `grafana/dashboard_otlp.json` | `alert_rules_otlp.yml` | Pushing via OTLP to an OTel Collector → Prometheus backend |

### Prometheus Rules

`alert_rules_common.yml` includes parser-health alerts
(`cm3500_parse_success` and per-section failures), modem availability, reboot,
and interface/event-log checks. Always load it, then choose **one**
plant-specific file:

- `alert_rules_eurodocsis.yml` — Vodafone DE, Liberty Global, Virgin Media O2, etc.
- `alert_rules_docsis_us.yml` — Comcast, Spectrum, Cox, etc.

#### Common Rules (`alert_rules_common.yml`)

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
| `CM3500DHCPRenewNoResponse` | warning | DHCP renew no-response events increasing |
| `CM3500UpstreamProfileDowngraded` | warning | Upstream profile ID decreased over 30m |
| `CM3500UpstreamPowerHigh` | warning | Upstream OFDMA power > 49 dBmV |
| `CM3500UpstreamPowerDeltaHigh` | warning | Upstream SC-QAM tilt > 6 dBmV |

### EuroDOCSIS Rules (`alert_rules_eurodocsis.yml`)

For European cable plants. Key thresholds:
- Downstream low power: **-5 dBmV** (European outlet spec 56–73 dBuV; plant tilt makes lower channels vulnerable)
- Upstream warning: **49 dBmV** (Vodafone DE plants degrade past 50 dBmV on bonded channels)
- Upstream critical: **53 dBmV** (EuroDOCSIS bonded-channel maximum)

| Alert | Severity | Condition |
|-------|----------|----------|
| `CM3500EuroDownstreamPowerLow` | warning | DS power < -5 dBmV for 5m |
| `CM3500EuroDownstreamPowerHigh` | warning | DS power > +12 dBmV for 5m |
| `CM3500EuroUpstreamPowerHigh` | warning | US SC-QAM power > 49 dBmV for 5m |
| `CM3500EuroUpstreamPowerCritical` | critical | US SC-QAM power > 53 dBmV for 5m |
| `CM3500EuroUpstreamOFDMAHigh` | warning | US OFDMA power > 49 dBmV for 5m |
| `CM3500EuroUpstreamOFDMACritical` | critical | US OFDMA power > 53 dBmV for 5m |

### US DOCSIS Rules (`alert_rules_docsis_us.yml`)

For North American cable plants. Key thresholds:
- Downstream low power: **-7 dBmV** (standard ISP operating window -7 to +7 dBmV)
- Upstream warning: **51 dBmV** (bonded 4ch ATDMA/64QAM maximum)
- Upstream critical: **55 dBmV** (well above bonded-channel limit)

| Alert | Severity | Condition |
|-------|----------|----------|
| `CM3500USDownstreamPowerLow` | warning | DS power < -7 dBmV for 5m |
| `CM3500USDownstreamPowerHigh` | warning | DS power > +12 dBmV for 5m |
| `CM3500USUpstreamPowerHigh` | warning | US SC-QAM power > 51 dBmV for 5m |
| `CM3500USUpstreamPowerCritical` | critical | US SC-QAM power > 55 dBmV for 5m |
| `CM3500USUpstreamOFDMAHigh` | warning | US OFDMA power > 51 dBmV for 5m |
| `CM3500USUpstreamOFDMACritical` | critical | US OFDMA power > 55 dBmV for 5m |

### Grafana Dashboards

#### Prometheus Dashboard (`grafana/dashboard.json`)

Sections:

- **Overview** — Status, uptime, DHCP lease gauge, BPI state, CPE count, Ethernet PHY, scrape duration, modem info
- **Downstream SC-QAM** — Power timeseries with DOCSIS thresholds, SNR timeseries, corrected/uncorrectable error bars, power tilt bar chart
- **Downstream OFDM** — RxMER timeseries (pilot/plc/data), channel details table
- **Upstream** — Power timeseries (QAM + OFDMA), power delta/tilt gauge, active profile, OFDMA profile ID, symbol rate
- **RF Spectrum** — Full-band spectrum analyzer, chunk power summary, product logging components
- **DHCP & Lease** — Lease/renew/rebind timers, DHCP renew no-response, QoS shaper gauge, service flow table
- **Event Log** — Critical event stats, event bar chart by ID, event trends over time
- **DOCSIS State** — Registration phases, QoS service flows
- **Collector Health** — Scrape duration timeline

#### OTLP Dashboard (`grafana/dashboard_otlp.json`)

Same panels and sections as the Prometheus dashboard, but uses OTLP metric names (e.g. `hw_cable_modem_downstream_power` instead of `cm3500_downstream_qam_power_dbmv`) and OTLP attribute labels (e.g. `cable_channel_id` instead of `channel`, `cable_channel_kind` for SC-QAM vs OFDMA).

Use this dashboard when the exporter pushes metrics via OTLP to an OTel Collector that writes to a Prometheus-compatible backend (Grafana Mimir, Thanos, Cortex).

### OTLP Alert Rules (`alert_rules_otlp.yml`)

A single combined file with all three groups (common + EuroDOCSIS + US DOCSIS) using OTLP metric names. All alert names are suffixed with `[OTLP]` in their summary to distinguish them from Prometheus-native alerts.

Load this file **instead of** the three Prometheus rule files when using an OTLP pipeline.

## Prometheus Configuration

For a host install, scrape the exporter's loopback listener. For the provided
Compose stack, use `exporter:10044` and keep the exporter port unpublished.

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
│   ├── dashboard.json           # Grafana dashboard (Prometheus metric names)
│   ├── dashboard_otlp.json      # Grafana dashboard (OTLP/OpenTelemetry metric names)
│   └── provisioning/            # Auto-provisioning configs
│       ├── dashboards/
│       └── datasources/
├── prometheus/
│   ├── prometheus.yml               # Prometheus config
│   ├── alert_rules_common.yml      # Common alerts (all regions)
│   ├── alert_rules_eurodocsis.yml   # EuroDOCSIS power thresholds
│   ├── alert_rules_docsis_us.yml   # US DOCSIS power thresholds
│   └── alert_rules_otlp.yml        # OTLP metric name alerts (all regions combined)
├── flake.nix                # Nix dev shell
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
└── README.md
```
