{
  description = "Oinori - Educational atomic arbitrage bot implementation on Aptos blockchain. This project demonstrates DEX arbitrage strategies, transaction construction, and MEV concepts for learning purposes.";

  inputs = {
    nixpkgs.url = "github:tacogips/nixpkgs-security/ae36ce5d71747ef2655933b315a361463ea90d82";
    nixpkgs-unstable.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-unstable,
      flake-utils,
      fenix,
    }:
    # Support ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"]
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ fenix.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
        pkgs-unstable = import nixpkgs-unstable { inherit system; };
        linuxLibraries = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
          pkgs.udev
          pkgs.elfutils
        ];
        rust-components = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          #sha256 = pkgs.lib.fakeSha256;
          sha256 = "sha256-opUgs6ckUQCyDxcB9Wy51pqhd0MPGHUVbwRKKPGiwZU=";
        };
      in

      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.binutils
            pkgs.gcc
            pkgs.lld
          ];

          buildInputs = [
            rust-components
            pkgs.zlib
            pkgs.git
            pkgs.llvmPackages.libclang.lib
            pkgs.openssl
            pkgs.zlib
            pkgs.postgresql.lib
            pkgs-unstable.nodejs_22
            pkgs.go-task
          ] ++ linuxLibraries;

          shellHook = ''
            docker buildx use default

            # https://discourse.nixos.org/t/rust-opencv-libllvm-16-so-cannot-open-shared-object-file/44524/4
            export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"

            # Prepend unstable libs for nodejs/claude-o compatibility (OPENSSL_3.4.0, CXXABI_1.3.15)
            export LD_LIBRARY_PATH="${
              pkgs.lib.makeLibraryPath ([
                pkgs-unstable.stdenv.cc.cc.lib
                pkgs-unstable.openssl
                pkgs-unstable.icu
                pkgs.zlib
                    pkgs.stdenv.cc.cc.lib
                pkgs.openssl
                    pkgs.postgresql.lib
              ] ++ linuxLibraries)
            }''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          '';
        };
      }
    );
}