{
  craneLib,
  lib,
}:

let
  pname = "gerrit-circus-bridge";
  root = ./..;

  src = craneLib.cleanCargoSource root;
  commonArgs = {
    inherit pname src;
    strictDeps = true;
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  "package-${pname}" = craneLib.buildPackage (
    commonArgs
    // {
      inherit pname;
      meta.mainProgram = pname;
    }
  );

  rust-clippy = craneLib.cargoClippy (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    }
  );

  rust-doc = craneLib.cargoDoc (
    commonArgs
    // {
      inherit cargoArtifacts;
      env.RUSTDOCFLAGS = "--deny warnings";
    }
  );

  rust-fmt = craneLib.cargoFmt {
    inherit cargoArtifacts src;
    rustFmtExtraArgs = "--config-path ${root}/.rustfmt.toml";
  };

  toml-fmt = craneLib.taploFmt {
    inherit cargoArtifacts;
    src = lib.sources.sourceFilesBySuffices src [ ".toml" ];
    taploExtraArgs = "--config ${root}/.taplo.toml";
  };
}
