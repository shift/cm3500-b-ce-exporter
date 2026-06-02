# Example systemd integration

These examples show how to consume the exporter's automation outputs:

- `/run/cm3500/link-state.json`
- `/run/cm3500/capacity.json`

They are intentionally policy-light and meant to be adapted for your own gateway setup.

## Files

- `cm3500-failover.path` - watches the link-state file
- `cm3500-failover.service` - oneshot service triggered when the state file changes
- `cm3500-failover-handler` - example handler that starts either a failover or recovery unit
- `cm3500-capacity.path` - watches the capacity file
- `cm3500-capacity.service` - oneshot service triggered when the capacity file changes
- `cm3500-apply-capacity` - example handler that reads shaped upstream/downstream rates

## Exporter example

```ini
ExecStart=/usr/bin/cm3500-exporter \
  --modem-url https://192.168.100.1 \
  --state-file /run/cm3500/link-state.json \
  --capacity-file /run/cm3500/capacity.json \
  --state-down-threshold 3 \
  --state-up-threshold 2 \
  --capacity-margin-percent 95
```

## Failover integration model

The example failover handler:

- reads `.status` from the JSON file
- starts `FAILOVER_TO_UNIT` when status is `down`
- starts `FAILBACK_TO_UNIT` when status is `up`
- ignores `degraded`

You should replace those unit names with the units that already exist in your gateway project.

## Capacity integration model

The example capacity handler:

- reads `shaped_upstream_bps` and `shaped_downstream_bps`
- writes them into `/run/cm3500/shaping.env`

That file can then be imported by your own shaping service, for example with:

```ini
EnvironmentFile=/run/cm3500/shaping.env
```

## Installing

Copy the files into place, for example:

- units -> `/etc/systemd/system/`
- helper scripts -> `/usr/local/libexec/`

Then update the script paths in the units if needed and enable the path units:

```bash
systemctl daemon-reload
systemctl enable --now cm3500-failover.path
systemctl enable --now cm3500-capacity.path
```
