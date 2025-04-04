mod async_runtime {
    // This is not precise, nor full, not even compilable and now its super minimal

    pub struct JoinHandle {}
    impl Future for JoinHandle {}
    impl JoinHandle {
        
        pub fn new() -> Self;

        ///
        /// True when Task has already completed
        /// 
        pub fn is_done() -> bool;

        ///
        /// Schedules abort to a Task. This means that:
        ///  - if task is not executing now, while scheduled for execution it will be immediately dropped
        ///  - if its running and will finish, returned value will be dropped
        /// 
        pub fn abort();

        ///
        /// Returns `id` of a Task that it is for
        /// 
        pub fn id();


        ///
        /// Sets the time limit for waiting on Task. When Task will not finish before it, 
        /// handle will be awaited with error
        /// 
        pub fn with_timeout(duration);
    }   




    ///
    /// Spawns an async function into runtime for execution.
    /// 
    /// JoinHandle - use to wait for result and fetch it
    /// 
    pub fn spawn(async_fn) -> JoinHandle {

    }

    ///
    /// Spawns an sync function into runtime for execution.
    /// 
    /// JoinHandle - use to wait for result and fetch it
    /// 
    pub fn spawn_fn(sync_fn) -> JoinHandle {

    }

    ///
    /// Spawns an async function into runtime for execution on specific worker.
    /// 
    /// JoinHandle - use to wait for result and fetch it
    /// 
    pub fn spawn_exclusive(async_fn, exclusive_worker_identifier) -> JoinHandle {

    }

    ///
    /// Spawns an sync function into runtime for execution on specific worker.
    /// 
    /// JoinHandle - use to wait for result and fetch it
    /// 
    pub fn spawn_exclusive_fn(sync_fn, exclusive_worker_identifier) -> JoinHandle {

    }


    /// Same versions for batches taking slice [Box<dyn Trait>] as input



}