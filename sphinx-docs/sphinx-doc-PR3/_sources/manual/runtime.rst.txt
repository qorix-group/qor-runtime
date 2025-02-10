
What is Runtime?
################

.. warning::

   This documentation is a work in progress and may be incomplete or subject to change.

The **Runtime** is a comprehensive framework designed to tackle the challenges of modern concurrent programming. It provides an orchestrator that enables deterministic, end-to-end execution control, allowing applications to be built with **flexibility**, **scalability**, **determinism**, and **performance** in mind. By **decoupling application logic from deployment details**, Runtime empowers developers to concentrate on the core logic of their applications, while abstracting away the complexities of low-level concurrency and resource management.

Task-Based Programming with Runtime
-----------------------------------

At the heart of the Runtime is a **deterministic workflow orchestration framework**, where developers define tasks and their execution flow using **actions**. Some of the actions and their use cases are:

Programs as Composable Task Flows
---------------------------------

Runtime allows developers to combine actions into an **execution flow**, forming a higher-level construct called a **program**. A program represents a complete workflow or logical unit of work, which itself is treated as a **task** within the framework. This recursive composition enables modularity and reuse. A program can be chained up or dependant on other programs by **Sync** and **Trigger** actions, facilitating the creation of complex and scalable systems.

Engine for Abstracting Computation Resources
--------------------------------------------

Once the program is defined, it is executed on a **Runtime engine**, which abstracts away the underlying computation resources.

See the section :ref:`runtime_concepts` for detailed explanation of Actions, Program and Engine.
