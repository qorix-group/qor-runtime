load("@crates//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_library")

rust_library(
    name = "libfoundation",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "foundation",
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ),
)
