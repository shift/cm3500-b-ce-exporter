{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, fenix }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          toolchain = fenix.packages.${system}.complete.toolchain;
        in {
          default = (pkgs.makeRustPlatform {
            rustc = toolchain;
            cargo = toolchain;
          }).buildRustPackage {
            pname = "cm3500-b-ce-exporter";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;

            src = self;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];

            meta = with pkgs.lib; {
              description = "Prometheus exporter for ARRIS CM3500B CE cable modem (DOCSIS 3.1 / EuroDOCSIS 3.0)";
              homepage = "https://github.com/shift/cm3500-b-ce-exporter";
              license = licenses.agpl3Only;
              maintainers = [ maintainers.shift ];
              mainProgram = "cm3500-b-ce-exporter";
            };
          };
        }
      );

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          toolchain = fenix.packages.${system}.complete.toolchain;
        in {
          default = pkgs.mkShell {
            packages = [
              toolchain
              pkgs.pkg-config
            ];

            RUST_SRC_PATH = "${fenix.packages.${system}.complete.rust-src}/lib/rustlib/src/rust/library";
          };
        }
      );
    };
}
