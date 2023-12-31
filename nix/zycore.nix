# https://github.com/NixOS/nixpkgs/blob/nixos-unstable/pkgs/development/libraries/zydis/zycore.nix

{ lib, stdenv, fetchFromGitHub, cmake }:

stdenv.mkDerivation rec {
  pname = "zycore";
  version = "1.4.1";

  src = fetchFromGitHub {
    owner = "zyantific";
    repo = "zycore-c";
    rev = "v${version}";
    hash = "sha256-kplUgrYecymGxz92tEU6H+NNtcN/Ao/tmmqdVo2c7HA=";
  };

  nativeBuildInputs = [ cmake ];

  # The absolute paths set by the Nix CMake build manager confuse
  # Zycore's config generation (which appends them to the package path).
  cmakeFlags =
    [ "-DCMAKE_INSTALL_LIBDIR=lib" "-DCMAKE_INSTALL_INCLUDEDIR=include" ];

  meta = {
    homepage = "https://zydis.re/";
    description = "Fast and lightweight x86/x86-64 disassembler library";
    license = lib.licenses.mit;
  };
}
