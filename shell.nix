{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell {
  name = "checker-shell";
  buildInputs = with pkgs; [
    rustc
    cargo
    clippy
    rustfmt
  ];
}