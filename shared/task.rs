use ::chrono::prelude::*;
use ::serde_derive::{Deserialize, Serialize};
use ::strum_macros::Display;

#[derive(Clone, Display, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Queued,
    Stashed,
    Running,
    Paused,
    Done,
    Failed,
}

/// Representation of a task.
/// start will be set the second the task starts processing.
/// exit_code, output and end won't be initialized, until the task has finished.
/// The output of the task is written into seperate files.
/// Upon task completion, the output is read from the files and put into the struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: i32,
    pub command: String,
    pub arguments: Vec<String>,
    pub path: String,
    pub status: TaskStatus,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub start: Option<DateTime<Local>>,
    pub end: Option<DateTime<Local>>,
}

impl Task {
    pub fn new(command: String, arguments: Vec<String>, path: String) -> Task {
        Task {
            id: 0,
            command: command,
            arguments: arguments,
            path: path,
            status: TaskStatus::Queued,
            exit_code: None,
            stdout: None,
            stderr: None,
            start: None,
            end: None,
        }
    }

    pub fn is_running(&self) -> bool {
        return self.status == TaskStatus::Running || self.status == TaskStatus::Paused;
    }

    pub fn is_done(&self) -> bool {
        return self.status == TaskStatus::Done || self.status == TaskStatus::Failed;
    }

    pub fn is_queued(&self) -> bool {
        return self.status == TaskStatus::Queued || self.status == TaskStatus::Stashed;
    }
}