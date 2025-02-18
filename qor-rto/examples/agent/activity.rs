// Define the Activity trait

use qor_rto::prelude::*;

use std::fmt::Debug;

use std::marker::Sync;

pub trait Activity: Send + Sync{
    fn init(&mut self) -> RoutineResult;
    fn step(&mut self) -> RoutineResult;
    fn terminate(&mut self) -> RoutineResult;
    fn getname(&mut self)-> String;
}

// Example Activity 1A
pub struct Activity1a {
    name: String,
}

impl Activity1a{
    pub fn new(named:String)->Activity1a{
        Self{name:named}
    }

}

impl Activity for Activity1a {



    fn init(&mut self) -> RoutineResult {
        println!("{}: Initializing...", self.name);
        RoutineResult::Ok(())
    }

    fn step(&mut self) -> RoutineResult {
        println!("{}: Executing step...", self.name);
        RoutineResult::Ok(())
    }

    fn terminate(&mut self) -> RoutineResult {
        println!("{}: Terminating...", self.name);
        RoutineResult::Ok(())
    }

    fn getname(&mut self)-> String{
        self.name.clone()
    }

}

// Example Activity 1B
pub struct Activity1b {
    name: String,
}

impl Activity1b{
    pub fn new(named:String)->Activity1b{
        Self{name:named}
    }

}

impl Activity for Activity1b {
    fn init(&mut self) -> RoutineResult {
        println!("{}: Initializing...", self.name);
        RoutineResult::Ok(())
    }

    fn step(&mut self) -> RoutineResult {
        println!("{}: Performing main task...", self.name);
        RoutineResult::Ok(())
    }

    fn terminate(&mut self) -> RoutineResult {
        println!("{}: Cleaning up...", self.name);
        RoutineResult::Ok(())
    }
    fn getname(&mut self)-> String{
        self.name.clone()
    }
}

// Example Activity 1C
pub struct Activity1c {
    name: String,
}

impl Activity1c{
    pub fn new(named:String)->Activity1c{
        Self{name:named}
    }

}

impl Activity for Activity1c {
    fn init(&mut self) -> RoutineResult {
        println!("{}: Setting up resources...", self.name);
        RoutineResult::Ok(())
    }

    fn step(&mut self) -> RoutineResult {
        println!("{}: Processing data...", self.name);
        RoutineResult::Ok(())
    }

    fn terminate(&mut self) -> RoutineResult {
        println!("{}: Shutting down...", self.name);
        RoutineResult::Ok(())
    }
    fn getname(&mut self)-> String{
        self.name.clone()
    }
}