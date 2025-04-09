load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_binary")

_COMMON_DEPS = [
    "//foundation:libfoundation",
    "//async_runtime:libasync_runtime",
    "//logging_tracing:liblogging_tracing",
] + all_crate_deps(
    normal = True
)

rust_library(
    name = "liborchestration",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "orchestration",
    visibility = ["//visibility:public"],
    deps = _COMMON_DEPS,
)

rust_binary(
    name = "basic",
    srcs = [
        "examples/basic.rs",
    ],
    visibility = ["//visibility:public"],
    deps = _COMMON_DEPS + [
         ":liborchestration",
    ],
)
