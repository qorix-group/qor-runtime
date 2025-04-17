# Qorix Runtime

![Build](https://github.com/qorix-group/performance_stack_rust/actions/workflows/cargo_build.yml/badge.svg)
![Test](https://github.com/qorix-group/performance_stack_rust/actions/workflows/cargo_test.yml/badge.svg)
![Format](https://github.com/qorix-group/performance_stack_rust/actions/workflows/cargo_fmt.yml/badge.svg)
![Clippy](https://github.com/qorix-group/performance_stack_rust/actions/workflows/cargo_clippy.yml/badge.svg)
![Lint](https://github.com/qorix-group/performance_stack_rust/actions/workflows/gitlint.yml/badge.svg)
![Miri](https://github.com/qorix-group/performance_stack_rust/actions/workflows/miri.yml/badge.svg)



## ⚠️ Disclaimer: Work in Progress (WIP)

Open sourcing for SCORE project.


This is the **Runtime implemented in Rust**. It is provided as
an open source library that is mainly focused on the automotive industry.

## Overview

This is the **Qorix Performance Stack implemented in Rust**. It is provided as
an open source library that is mainly focused on the automotive industry.

It contains the runtime orchestration `rto`, the `core` and the `os` layer.


## Documentation

See [User Manual](qor-rto/docs/source/manual/user_manual.rst).


## Examples

To get started these examples are provided:

  * `add` - example of a addition with our async_runtime
  * `sub` - example of a subtraction with our orchestrator

The examples can be started by using `cargo`:
```sh
cargo run --example add
```


## Getting started

### Manual method

[Install Rust](https://www.rust-lang.org/tools/install)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Run the example

### Using Cargo
```bash
cargo run --example basic
```

### Using Bazel
```bash
bazel run //orchestration:basic
```

### Devenv method for Nix environments

[Devenv](https://devenv.sh/) is a tool that provides a fast, declarative,
reproducible and composable developer environments using
[Nix](https://nixos.org/) known from [NixOS](https://nixos.org/).

Devenv allows to simply declare the build environment using a JSON-like
configuration file. It also creates a lock-file that stores the exact versions
of the environment, like the Rust compiler, so every developer has the same
setup similar to [Pythons venv](https://docs.python.org/3/library/venv.html).

It also configures [nix-direnv](https://direnv.net/) to automatically enter the
environment when the directory is changed to it. After the directory is left,
the environment is left and doesn't touch the rest of the system. This is only
available when using a compatible
[shell hook](https://direnv.net/docs/hook.html).

#### Advantages using devenv

  * simple tool and language configuration
  * doesn't clutter the system with project-specific tools and compilers
  * keeps track of tool and compiler versions
  * doesn't pollute the `PATH` environment, everything is kept inside the shell session

#### How to use it

After Nix and nix-direnv is installed and the matching shell-hook is in place
it can be activated by entering the cloned directory. On the first run it will
show a message that the directory must first be allowed by entering `direnv
allow`. After this, each time the directory is entered the environment will
automatically be available. It is also safe to use multiple shells in the same
directory.


## Git

### rustfmt git pre-commit hook

It is desireable that the code is formatted according to the rules of
[rustfmt](https://github.com/rust-lang/rustfmt). There is also a very nice
[rust style guide](https://doc.rust-lang.org/nightly/style-guide/). We provide a
pre-commit script that checks what you staged in git on a file basis with rustfmt.
You can activate this hook with

```bash
cp .githooks/pre-commit .git/hooks/
```

It prevents you from accidentially commiting code, that is not formatted according
to the rustfmt rules.


## Legals

Distributed under the [Apache 2.0 License](LICENSE).
