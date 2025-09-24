{
  description = "Stalwart";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs) lib;
      inherit (inputs) self;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      eachSystem = lib.genAttrs systems;
      pkgsFor = inputs.nixpkgs.legacyPackages;

      version = "0.13.2";

      fs = lib.fileset;
      src = fs.toSource {
        root = ./.;
        fileset = fs.difference (fs.gitTracked ./.) (
          fs.unions [
            ./.envrc
            ./flake.lock
            ./FUNDING.json
            ./README.md
            ./Dockerfile
            (fs.fileFilter (file: lib.strings.hasInfix ".git" file.name) ./.)
            (fs.fileFilter (file: file.hasExt "nix") ./.)
          ]
        );
      };
    in
    {
      devShells = eachSystem (
        system:
        let
          pkgs = pkgsFor.${system};
        in
        {
          default = pkgs.stdenvNoCC.mkDerivation {
            name = "stalwart-dev";
            src = null;
            buildInputs = with pkgs; [
              cargo
              rustc
              clippy
              rust-analyzer
              rustfmt

              llvm
              clang
              libclang
              foundationdb
              foundationdb.dev
              lld
              openssl
              openssl.dev

              nixfmt
            ];

            shellHook = ''
              echo "Stalwart Dev Environment"
              echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
              echo "Version: ${version}"
            '';

            env = {
              CC = "clang";
              CXX = "clang++";
              AR = "llvm-ar";
              RUSTFLAGS = "-C linker=clang -C link-arg=-fuse-ld=lld";
              RUST_BACKTRACE = 1;
              LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
              LD_LIBRARY_PATH = lib.makeLibraryPath [
                pkgs.openssl
                pkgs.foundationdb
                pkgs.stdenv.cc.cc.lib
              ];
              PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
            };
          };
        }
      );
    };
}
