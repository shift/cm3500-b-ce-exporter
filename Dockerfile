FROM nixos/nix:latest AS builder

# Enable flakes
RUN mkdir -p /etc/nix && echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

WORKDIR /app
COPY . .

# Build a fully static binary so it can run in a minimal distroless image.
RUN nix build .#static --no-link --print-out-paths > /out-path
RUN store_path=$(cat /out-path) && cp ${store_path}/bin/cm3500-b-ce-exporter /cm3500-b-ce-exporter

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=builder /cm3500-b-ce-exporter /cm3500-b-ce-exporter
EXPOSE 10044
USER nonroot:nonroot
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD ["/cm3500-b-ce-exporter", "--check-ready", "http://127.0.0.1:10044/ready"]
ENTRYPOINT ["/cm3500-b-ce-exporter"]