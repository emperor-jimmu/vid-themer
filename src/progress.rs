// Progress reporting and user feedback

use crate::processor::ProcessResult;

pub struct ProgressReporter {
    pub total: usize,
    pub current: usize,
    pub successful: usize,
    pub failed: usize,
}

impl ProgressReporter {
    pub fn new() -> Self {
        Self {
            total: 0,
            current: 0,
            successful: 0,
            failed: 0,
        }
    }

    pub fn start(&mut self, total: usize) {
        self.total = total;
        println!("Found {} videos to process", total);
    }

    pub fn update(&mut self, result: &ProcessResult) {
        self.current += 1;
        
        println!("[{}/{}] Processing: {}", 
            self.current, 
            self.total, 
            result.video_path.display()
        );
        
        if result.success {
            self.successful += 1;
            println!("  → Output: {}", result.output_path.display());
        } else {
            self.failed += 1;
            if let Some(error) = &result.error_message {
                println!("  ✗ Error: {}", error);
            }
        }
    }

    pub fn finish(&self) {
        println!("Completed: {} successful, {} failed", self.successful, self.failed);
    }
}
