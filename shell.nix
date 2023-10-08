{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell rec {
  NIX_SHELL_NAME = "orchestrator";
  buildInputs = with pkgs; [
    openssl
    pkg-config

    libxkbcommon
    libGL
    clang_16
    mold

    # WINIT_UNIX_BACKEND=wayland
    wayland
    # https://github.com/emilk/egui/discussions/1587
    # WINIT_UNIX_BACKEND=x11
    #xorg.libXcursor
    #xorg.libXrandr
    #xorg.libXi
    #xorg.libX11
  ];

  shellHook = ''
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="${pkgs.clang_16.out}/bin/clang"
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=--ld-path=${pkgs.mold.out}/bin/mold"
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath buildInputs}"
    export OPENSSL_DIR=${pkgs.openssl.dev}
    export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
    export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
    export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig
  '';
}
