{
  pkgs,
  lib,
  config,
  inputs,
  ...
}: {
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = [
    pkgs.gnumake
    pkgs.llvmPackages.clangUseLLVM
    pkgs.sphinx
  ];

  env = {
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    BINDGEN_EXTRA_CLANG_ARGS = "-I ${pkgs.linuxHeaders}/include -I ${pkgs.glibc.dev}/include";
  };
}
