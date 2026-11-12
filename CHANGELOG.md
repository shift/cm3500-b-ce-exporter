# Changelog

## v0.2.2

### Fixed

- Added exporter-side monotonic event counters so event alerts use `increase()`
  on real counters instead of `changes()` on rolling-window gauges.
- OTLP pushes the equivalent monotonic event counters.
- Docker image healthcheck via `--check-ready`.

## v0.2.1

### Fixed

- Emit each Prometheus metric family's `# HELP`/`# TYPE` exactly once. The
  previous capacity/L2VPN/DHCP-attempt rendering could make the whole scrape
  invalid.
- Repaired the malformed `CM3500DHCPLeaseExpiring` alert rule.
- Corrected OTLP integer fields to `asInt` and removed duplicate OTLP metric
  names.
- OTLP now reports failure status, has request timeouts, and marks log records
  sent only after a successful POST.
- OTLP counter start times stay stable across uptime jitter and advance on a
  detected modem reboot. `system.uptime` is emitted as a gauge.
- The Docker image now builds a static binary for distroless.
- The HTTP listener binds before the first modem scrape, so `/health` and
  `/ready` are available while the modem is unreachable.
- Optional modem pages are best-effort; missing `dsx_cgi` or log pages no longer
  discard core RF telemetry.
- Session expiry is checked on every page, and redirect-based logins are handled.
- Spectrum bin allocation is bounded, failed scans respect the scan interval,
  and scan age is exported.
- Removed the high-churn `time_and_date` label and unbounded scrape-error label.
- Added exporter-side monotonic event counters (`cm3500_event_*_total` and
  OTLP equivalents); event alerts now use `increase()` instead of `changes()`
  on rolling-window gauges.
- Debug bundles are pruned to the ten most recent failures.
- Prometheus and OTLP metrics now include parser-health, capacity, DHCPv6 and
  DSX coverage; dashboards and alerts were updated accordingly.

### Breaking changes from v0.1.x

- Default `--listen` is now `127.0.0.1:10044` (previously `0.0.0.0:10044`).
- QoS rate metrics are now `*_bps` and use bits per second, not kbps.
- TLS verification is enabled by default; use `--insecure-tls` or
  `--tls-cert-fingerprint` for the CM3500B's fixed certificate.
- Compose no longer publishes the exporter port and requires
  `GRAFANA_ADMIN_PASSWORD`.
- `cm3500_scrape_error` is now an unlabeled gauge; the error text is logged.
- `cm3500_status_info` no longer includes the churning `time_and_date` label.

## v0.2.0

- Initial complete modem coverage: all reachable pages, DHCPv6 log, DSX log,
  parser health, readiness, TLS pinning, OTLP parity, and failure-path tests.