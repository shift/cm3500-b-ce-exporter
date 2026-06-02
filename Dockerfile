FROM nixos/nix:latest AS builder

# Enable flakes
RUN mkdir -p /etc/nix && echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

WORKDIR /app
COPY . .

# Build the package
RUN nix build . --no-link --print-out-paths > /out-path
RUN store_path=$(cat /out-path) && cp ${store_path}/bin/cm3500-b-ce-exporter /cm3500-b-ce-exporter

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /cm3500-b-ce-exporter /cm3500-b-ce-exporter
EXPOSE 10044
ENTRYPOINT ["/cm3500-b-ce-exporter"]
