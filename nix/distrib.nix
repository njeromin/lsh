{inputs, ...}: {
  perSystem = {
    self',
    pkgs,
    system,
    ...
  }: let
    toolchain = with inputs.fenix.packages.${system};
      combine [
        minimal.rustc
        minimal.cargo
        targets.x86_64-unknown-linux-gnu.latest.rust-std
        targets.x86_64-unknown-linux-musl.latest.rust-std
        targets.x86_64-pc-windows-gnu.latest.rust-std
        # targets.i686-pc-windows-gnu.latest.rust-std
      ];

    naersk' = pkgs.callPackage inputs.naersk {
      cargo = toolchain;
      rustc = toolchain;
    };

    naerskPkg = target: args:
      naersk'.buildPackage (
        args
        // {
          src = ../.;

          # doCheck = true;

          CARGO_BUILD_TARGET = target;
        }
      );
  in {
    packages = {
      default = self'.packages.x86_64-unknown-linux-gnu;
      x86_64-unknown-linux-gnu = naerskPkg "x86_64-unknown-linux-gnu" {
        nativeBuildInputs = with pkgs; [pkgsStatic.stdenv.cc];
      };
      x86_64-unknown-linux-musl = naerskPkg "x86_64-unknown-linux-musl" {
        nativeBuildInputs = with pkgs; [pkgsStatic.stdenv.cc];
        CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS = "-C target-feature=+crt-static";
      };
      x86_64-pc-windows-gnu = naerskPkg "x86_64-pc-windows-gnu" {
        strictDeps = true;

        depsBuildBuild = with pkgs; [
          pkgsCross.mingwW64.stdenv.cc
          pkgsCross.mingwW64.windows.pthreads
        ];

        nativeBuildInputs = with pkgs; [
          wineWowPackages.stable
        ];

        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER = pkgs.writeScript "wine-wrapper" ''
          export WINEPREFIX="$(mktemp -d)"
          exec wine64 $@
        '';
      };
    };
  };
}
