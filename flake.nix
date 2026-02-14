{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs = {
    nixpkgs,
    rust-overlay,
    ...
  }: let
    pkgs = import nixpkgs {
      system = "x86_64-linux";
      overlays = [(import rust-overlay)];
    };
    rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    nativeBuildInputs = with pkgs; [rust-toolchain clang mold pkg-config];
    buildInputs = with pkgs; [ffmpeg_7-headless libclang];
  in {
    devShells.x86_64-linux.default = pkgs.mkShell {
      inherit nativeBuildInputs buildInputs;
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
    };
  };
}
