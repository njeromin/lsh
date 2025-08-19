{inputs, ...}: {
  perSystem = {system, ...}: {
    checks.pre-commit = inputs.pre-commit-hooks.lib.${system}.run {
      src = ./.;
      hooks = {
        alejandra.enable = true;
      };
    };
  };
}
