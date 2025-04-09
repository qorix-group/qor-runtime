load("@rules_rust//rust:defs.bzl", "rust_library")
load("@crates//:defs.bzl", "aliases", "all_crate_deps")

rust_library(
    name = "liblogging_tracing",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "logging_tracing",
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ),
)
