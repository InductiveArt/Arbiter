#[derive(Debug)]
pub struct TaskFailure {
    pub reason: String,
}

impl From<String> for TaskFailure {
    fn from(reason: String) -> Self {
        TaskFailure { reason }
    }
}

impl From<&str> for TaskFailure {
    fn from(reason: &str) -> Self {
        TaskFailure {
            reason: reason.to_string(),
        }
    }
}
