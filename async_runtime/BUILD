load("@crates//:defs.bzl", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_library")

rust_library(
    name = "libasync_runtime",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "async_runtime",
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ) + [
        "//foundation:libfoundation",
    ],
)
