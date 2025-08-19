{
  perSystem = {
    self',
    pkgs,
    ...
  }: let
    mkShell = args:
      pkgs.mkShell (pkgs.lib.attrsets.recursiveUpdate args {
        inherit (self'.checks.pre-commit) shellHook;
        buildInputs = self'.checks.pre-commit.enabledPackages;
      });
  in {
    devShells.default = mkShell {
      inputsFrom = [pkgs.rustc pkgs.cargo];
    };
  };
}
