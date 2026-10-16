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
pub(crate) struct ExecutionStream {
    pub run: u64,
    pub asset: usize,
    pub next_execution: u64,
    pub next_response: u64,
    pub pending: Vec<Execution>,
    pub observer: Box<dyn ExecutionObserver>,
}
pub(crate) fn failure(message: &str) -> BacktestError {
    BacktestError::DataError(std::io::Error::other(message.to_owned()))
}
