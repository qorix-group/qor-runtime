Getting Started
###############


General
=======

To execute a simple task using the Runtime framework, start by creating a Routine and passing it to generate a task. Next, initialize an engine responsible for running the task and spawn the task on the engine. Finally, wait for the task to complete.


A first example : Hello World
=============================

Introduction
-------------
"Hello World!" is often the first program written. This example demonstrates how to print "Hello World!" using the Runtime APIs.

Code Walkthrough
-----------------
At first we import with **use** keyword to get the required qor-runtime items into scope.

.. code-block:: rust
    
    //Imports the required qor-runtime items to be used in this scope
    use qor_rto::prelude::*;

Write a simple async function which prints "Hello World!".

.. code-block:: rust

    //simple rust function to print hello world
    async fn hello_world() {
        println!("Hello World!"); // Hello the world
    }

Next, we create an engine with the default configuration to run the task. In this case, the task takes the `hello_world()` function and runs it on the engine.

.. code-block:: rust

    // The engine is the central Runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // Create a new task from our hello_world routine
    let task = Task::new(hello_world);

    // Spawn the task on the engine
    let handle = engine.spawn(task).unwrap();


Finally, we wait for the task to complete before shutting down the engine.

.. code-block:: rust

    // Wait for the task to finish
    let _ = handle.join().unwrap();

    // Engine shutdown
    engine.shutdown().unwrap();


How to Run the Code
-------------------

.. code-block:: bash
    
    cargo run --example hello_world

Installation
------------

Refer to readme.