//! Opt-in, bounded execution delivery for the T1 integration profile.
//! IDs below belong to this extension, not the upstream exchange model.
use crate::{
    backtest::BacktestError,
    types::{Order, Side},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Execution {
    pub run: u64,
    pub asset: usize,
    pub order_id: u64,
    pub generation: u64,
    pub revision: u64,
    pub execution_id: u64,
    pub side: Side,
    pub qty: f64,
    pub price_tick: i64,
    pub tick_size: f64,
    pub maker: bool,
    pub exchange_timestamp: i64,
    pub leaves_before: f64,
    pub leaves_after: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseEnvelope {
    pub response_id: u64,
    pub executions: Vec<Execution>,
}
pub trait ExecutionObserver {
    fn generated(&mut self, execution: &Execution) -> Result<(), BacktestError>;
    fn registered(
        &mut self,
        order: &Order,
        envelope: &ResponseEnvelope,
        arrival: i64,
    ) -> Result<(), BacktestError>;
}
pub trait ExecutionConsumer {
    fn received(
        &mut self,
        order: &Order,
        envelope: &ResponseEnvelope,
        arrival: i64,
    ) -> Result<(), BacktestError>;
}
/// Explicit cumulative delivery budgets, separate from bounded in-flight storage.
/// The legacy observer API keeps 16 executions / 32 responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub executions: u64,
    pub responses: u64,
    pub pending_executions: usize,
    pub queued_responses: usize,
}
impl Default for ExecutionLimits {
    fn default() -> Self { Self { executions: 16, responses: 32, pending_executions: 16, queued_responses: 32 } }
}
impl ExecutionLimits {
    pub fn validate(&self) -> Result<(), BacktestError> {
        if self.executions == 0 || self.responses == 0 || self.executions == u64::MAX || self.responses == u64::MAX
            || self.pending_executions == 0 || self.pending_executions > 16
            || self.queued_responses == 0 || self.queued_responses > 32
            || self.pending_executions as u64 > self.executions || self.queued_responses as u64 > self.responses {
            return Err(failure("invalid_execution_limits"));
        }
        Ok(())
    }
    /// Admission used by the native extension before assigning an execution ID.
    pub fn admit_execution(&self, next: u64, pending: usize) -> Result<(), BacktestError> {
        if next == 0 || next > self.executions || next.checked_add(1).is_none() || pending >= self.pending_executions {
            return Err(failure("execution_capacity_or_quantity"));
        }
        Ok(())
    }
    /// Admission used before appending the response envelope to the existing bus.
    pub fn admit_response(&self, next: u64, queued: usize) -> Result<(), BacktestError> {
        if next == 0 || next > self.responses || next.checked_add(1).is_none() || queued >= self.queued_responses {
            return Err(failure("response_capacity_or_order"));
        }
        Ok(())
    }
}
pub(crate) struct ExecutionStream {
    pub run: u64,
    pub asset: usize,
    pub next_execution: u64,
    pub next_response: u64,
    pub pending: Vec<Execution>,
    pub limits: ExecutionLimits,
    pub observer: Box<dyn ExecutionObserver>,
}
pub(crate) fn failure(message: &str) -> BacktestError {
    BacktestError::DataError(std::io::Error::other(message.to_owned()))
}
