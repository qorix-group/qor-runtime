load("@crates//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_library")

rust_library(
    name = "liborchestration",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "orchestration",
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ) + [
        "//foundation:libfoundation",
        "//async_runtime:libasync_runtime",
        "//logging_tracing:liblogging_tracing",
    ],
)
