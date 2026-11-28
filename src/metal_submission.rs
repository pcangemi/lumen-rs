//! Bound outstanding GPU work and drain it before shared buffers can be reused.
use metal::{CommandBuffer, CommandBufferRef};

pub(super) struct SubmittedBatch(Vec<CommandBuffer>);
impl SubmittedBatch {
    pub(super) fn new(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
    pub(super) fn push(&mut self, command: &CommandBufferRef) {
        self.0.push(command.to_owned());
    }
    pub(super) fn commands(&self) -> &[CommandBuffer] {
        &self.0
    }
}
impl Drop for SubmittedBatch {
    fn drop(&mut self) {
        // Also runs on an error or early return. Camera resets must never race an
        // outstanding write, and resources must outlive their submitted commands.
        for command in &self.0 {
            command.wait_until_completed();
        }
    }
}
