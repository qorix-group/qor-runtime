use qor_rto::prelude::*;
///object detection
pub struct ObjectDetection {}

impl ObjectDetection {
    pub fn new() -> Self {
        Self {}
    }

    pub fn pre_processing(&mut self) -> RoutineResult {
        println!("PreProcessing start");
        Ok(())
    }
    pub fn drive_q1(&mut self) -> RoutineResult {
        println!("DriveQ1 start");
        Ok(())
    }
    pub fn drive_q2(&mut self) -> RoutineResult {
        println!("DriveQ2 start");
        Ok(())
    }
    pub fn drive_q3(&mut self) -> RoutineResult {
        println!("DriveQ3 start");
        Ok(())
    }
    pub fn object_fusion(&mut self) -> RoutineResult {
        println!("ObjectFusion start");
        Ok(())
    }
}
