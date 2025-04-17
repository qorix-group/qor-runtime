
# Error handling in orchestration

## Introduction
Under normal circumstances, the task chain execution in the orchestration module shall execute according to the provided flow within the required timing and have enough resources to complete its functionality. However, It's hard or even impossible to predict, test, and mitigate all possible scenarios in a field that can influence such execution. That is why, orchestration needs to provide means to `monitor`, `detect`, and `react` to different kinds of errors:
- panics in user components within the task-chain
- timing violation of tasks
- error propagation from any task in the task-chain

If the program is part of an `ASIL` product, the above means need to guarantee by design that `reactions` can be executed even though other parts of the program can misbehave (infinite loops leading to threads being stuck, panicked threads, and so on).



## Identified scenarios for error handling 

- Task chain component exceeded its time budget and the error reaction shall be called
- Task chain component returns the error in some part of the task chain, which shall be propagated
- Task chain component panicked/thrown


## Handling in Program
To handle identified scenarios, orchestration provides the following means:
- `Catch` action - specific action that can be put inside Task Chain to monitor errors
- `Program` as the final error catch point in the Task Chain


### Catch action behavior
When the `Catch` action is inserted into the Task Chain, the user is allowed to choose which kind of errors will be handled by this action (error returns, timeouts, etc.). Choosing an error is called `ErrorFilter`.

- `TimeoutErrorFilter` will make sure that everything that is under `Catch` action will be executed below provided `timeout` and otherwise will call user provided error handler.
- `ReturnErrorFilter` will call user provided error handler once it get error returned from the contained action

During error handler execution, the user can execute their own code and decide whether to continue execution from this point or propagate the error down the chain.
Any propagation is done to the closest `Catch` action and there user can re-propagate it further as below:

```plantuml
partition ProgramIteration {
 start
    note right
 This is the build-in catch point for all errors within Task Chain.
 Once error reach this place, program execution is terminated with 
 corresponding error
    end note

 :Sequence;
 :Catch;

    note right
 As before, the user can resume from here or propagate to **Program** where it will finish with termination.
    end note

 partition CatchBlock {
 :Concurrency;

 partition ConcurrencyBlock {
 split
 :Action1;
 split again
 :Catch2;

            note right
 This is user provided catch point for errors. IT will only recognize errors
 that match the provided **Filter** and will call the connected handler. User can continue the task chain consuming error
 or propagate it down the chain (in this case, to the previous **Catch**) 
            end note
            
 partition CatchBlock {
 :Action2;
 }

 split again
 :ActionN;
 end split
 }
   
 :Sequence;
 }

 :Sequence;

 end
}

```

## How does async_runtime facilitate Orchestration error handling

### Cooperative Mode - ie. non QM/QM
In this mode, error handling can only react after a worker has completed its current task. This conserves thread resources by not dedicating an extra thread for error handling, but it means that if all tasks become blocked (for example, stuck in an infinite loop), the error handling can only react until a task finishes or yields (error handling tasks are treated like other tasks).

For this case `async_runtime` does not need to offer any special means of handling errors. The implementation of `orchestration` has to handle itself correctly errors within its code and propagate it through the task chain according to its `requirements`. Also for timeout monitoring, `async_runtime` provides an API to register `task wake-ups` after a specified amount of time (ie. async sleeps. timers).

As this part is not a safety concern there is no guarantee where and if the given error action will be scheduled and it solely depends on overall application validity and the assumption it **DOES NOT** misbehave in a way it could put `async_runtime` into being not responsive.

### Preemptive Mode - ie. ASIL N
In this mode, an additional thread is allocated specifically to execute error handling mechanisms. This ensures immediate reactivity even if all worker threads are busy or stuck, providing a higher level of reliability in scenarios where timely error response is critical.

For this case `async_runtime`  **SHOULD** provide technical solution to guarantee that:
- when the user request a `safety` timeout monitoring for some task, the wake-up of monitoring functionality **SHOULD NOT** be impacted by
any potential misbehavior of tasks already running in `async_runtime`
- same for errors returned from tasks
- same for panics caught in tasks


#### High Level Description & Requirements
To fulfill the above, the below requirements were specified:

> **Requirement:** The async_runtime should create **SAFETY WORKER** which runs on it's own **OS THREAD** with configurable *priority* and *affinity*

> **Requirement:** The async_runtime shall provide *safety waker* that let higher level implementation mark *wake-up* as safety relevant and schedule it within *SAFETY WORKER*

> **Requirement:** The async_runtime should not schedule any work into this worker unless it's scheduled by *safety waker*

> **Requirement:** The async_runtime should run *Timewheel* and/or *Timers* on **SAFETY WORKER** to provide reliable timeout monitoring for safety case

> **Requirement:** The async_runtime should wake-up connected task into **SAFETY WORKER** via *JoinHandle* if the task have returned *error* marked as *safety error*

> **Requirement:** The async_runtime should catch `panic` from user tasks and treat them as `errors`

Also currently we impose a small safety manual when using `safety` features of `async_runtime`:

> **Safety manual:** The user of *safety waker* should make sure that the code being woken up as an error handler, thus running on **SAFETY WORKER** will never block execution

> **Safety manual:** The `async_runtime` **SAFETY WORKER** periodic `keep alive` notification shall be monitored by external `Health` mechanism  (WIP: do we need this ?)

Below we describe the most important scenarios and the way they are handled:

##### async_runtime workers can be stuck

![WorkersStuck](./assets/error_handling_case_1.drawio.svg)

##### async_runtime can be stuck but after reschedule

![WorkersStuck](./assets/error_handling_case_2.drawio.svg)