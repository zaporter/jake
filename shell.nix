{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell {
  NIX_SHELL_NAME = "orchestrator";
  buildInputs = [
    pkgs.openssl
    pkgs.pkg-config
  ];

  shellHook = ''
    export OPENSSL_DIR=${pkgs.openssl.dev}
    export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
    export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
    export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig
  '';
}
