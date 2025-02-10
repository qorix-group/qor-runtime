# Qorix Runtime

![Build](https://github.com/qorix-group/performance_stack_rust/actions/workflows/rust.yml/badge.svg)
![Format](https://github.com/qorix-group/performance_stack_rust/actions/workflows/rustfmt.yml/badge.svg)
![Lint](https://github.com/qorix-group/performance_stack_rust/actions/workflows/gitlint.yml/badge.svg)
![Miri](https://github.com/qorix-group/performance_stack_rust/actions/workflows/miri.yml/badge.svg)


## Overview

This is the **Runtime implemented in Rust**. It is provided as
an open source library that is mainly focused on the automotive industry.

It contains the runtime orchestration `rto`, the `core` and the `os` layer.


## Documentation

See [Getting started](qor-rto/docs/source/manual/getting_started.rst).

See [Concepts](qor-rto/docs/source/manual/concepts.rst).


## Examples

To get started these examples are provided:

  * `camera_driver` - Camera image processor example
  * `camera_drv_object_det` - Camera object detection example
  * `object_detection` - Object detection example
  * `hello_async_world` - Async example
  * `hello_loop` - Loop example
  * `hello_program` - Program with action
  * `hello_task_result` - Spawn a task and get the result
  * `hello_tasks` - Spawn multiple tasks and join them
  * `hello_world` - Create a task and print `Hello World`
  * `loop_concurrency` - Show loop concurrency
  * `pt1_control` - PT1 control
  * `task_chain` - Task chain example
  * `thread_stress` - Load and stress example

The examples can be started by using `cargo`:

```sh
cargo run --example pt1_control
```


## Getting started

### Manual method

[Install Rust](https://www.rust-lang.org/tools/install)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Run the example

```bash
cargo run --example hello_world
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
