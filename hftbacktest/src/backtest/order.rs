use std::{cell::UnsafeCell, collections::VecDeque, rc::Rc};

use crate::{
    backtest::{
        BacktestError,
        execution::{Execution, ExecutionLimits, ExecutionObserver, ExecutionStream, ResponseEnvelope, failure},
        models::LatencyModel,
    },
    types::Order,
};

/// Provides a bus for transporting backtesting orders between the exchange and the local model
/// based on the given timestamp.
#[derive(Clone, Debug, Default)]
pub struct OrderBus {
    order_list: Rc<UnsafeCell<VecDeque<(Order, i64, Option<ResponseEnvelope>)>>>,
}

impl OrderBus {
    /// Constructs an instance of ``OrderBus``.
    pub fn new() -> Self {
        Default::default()
    }

    /// Returns the timestamp of the earliest order in the bus.
    pub fn earliest_timestamp(&self) -> Option<i64> {
        unsafe { &*self.order_list.get() }
            .front()
            .map(|(_order, ts, _)| *ts)
    }

    /// Appends the order to the bus with the timestamp.
    ///
    /// To prevent the timestamp of the order from becoming disordered, it enforces that the given
    /// timestamp must be equal to or greater than the latest timestamp in the bus.
    ///
    /// In crypto exchanges that use REST APIs, it may be still possible for order requests sent
    /// later to reach the matching engine before order requests sent earlier. However, for the
    /// purpose of simplifying the backtesting process, all requests and responses are assumed to be
    /// in order.
    pub fn append(&mut self, order: Order, timestamp: i64) {
        self.append_envelope(order, timestamp, None);
    }

    fn append_envelope(
        &mut self,
        order: Order,
        timestamp: i64,
        envelope: Option<ResponseEnvelope>,
    ) -> i64 {
        let latest_timestamp = {
            let order_list = unsafe { &*self.order_list.get() };
            let len = order_list.len();
            if len > 0 {
                let (_, timestamp, _) = order_list.get(len - 1).unwrap();
                *timestamp
            } else {
                0
            }
        };
        let timestamp = timestamp.max(latest_timestamp);
        unsafe { &mut *self.order_list.get() }.push_back((order, timestamp, envelope));
        timestamp
    }

    /// Resets this to clear it.
    pub fn reset(&mut self) {
        unsafe { &mut *self.order_list.get() }.clear();
    }

    /// Returns the number of orders in the bus.
    pub fn len(&self) -> usize {
        unsafe { &*self.order_list.get() }.len()
    }

    /// Returns ``true`` if the ``OrderBus`` is empty.
    pub fn is_empty(&self) -> bool {
        unsafe { &*self.order_list.get() }.is_empty()
    }

    /// Removes the first order and its timestamp and returns it, or ``None`` if the bus is empty.
    pub fn pop_front(&mut self) -> Option<(Order, i64)> {
        unsafe { &mut *self.order_list.get() }
            .pop_front()
            .map(|(o, t, _)| (o, t))
    }
}

/// Provides a bidirectional order bus connecting the exchange to the local.
pub struct ExchToLocal<LM> {
    to_exch: OrderBus,
    to_local: OrderBus,
    order_latency: LM,
    execution: Option<ExecutionStream>,
}

impl<LM> ExchToLocal<LM>
where
    LM: LatencyModel,
{
    pub fn with_execution_observer(
        mut self,
        run: u64,
        asset: usize,
        observer: Box<dyn ExecutionObserver>,
    ) -> Self {
        self.execution = Some(ExecutionStream {
            run,
            asset,
            next_execution: 1,
            next_response: 1,
            pending: Vec::with_capacity(16),
            limits: ExecutionLimits::default(),
            observer,
        });
        self
    }

    /// Opt-in cumulative budgets; matching, delivery timestamps and the consumer are unchanged.
    pub fn with_execution_observer_limits(
        mut self, run: u64, asset: usize, observer: Box<dyn ExecutionObserver>, limits: ExecutionLimits,
    ) -> Result<Self, BacktestError> {
        limits.validate()?;
        self.execution = Some(ExecutionStream { run, asset, next_execution: 1, next_response: 1,
            pending: Vec::with_capacity(limits.pending_executions), observer, limits });
        Ok(self)
    }

    pub(crate) fn prepare_execution(&self, order: &Order, qty: f64) -> Result<(), BacktestError> {
        if let Some(stream) = &self.execution {
            stream.limits.admit_execution(stream.next_execution, stream.pending.len())?;
            if !qty.is_finite()
                || qty <= 0.0
                || !order.leaves_qty.is_finite()
                || qty > order.leaves_qty
            {
                return Err(failure("execution_capacity_or_quantity"));
            }
        }
        Ok(())
    }

    pub(crate) fn record_execution(
        &mut self,
        order: &Order,
        leaves_before: f64,
    ) -> Result<(), BacktestError> {
        if let Some(stream) = &mut self.execution {
            let execution = Execution {
                run: stream.run,
                asset: stream.asset,
                order_id: order.order_id,
                generation: 1,
                revision: 1,
                execution_id: stream.next_execution,
                side: order.side,
                qty: order.exec_qty,
                price_tick: order.exec_price_tick,
                tick_size: order.tick_size,
                maker: order.maker,
                exchange_timestamp: order.exch_timestamp,
                leaves_before,
                leaves_after: order.leaves_qty,
            };
            stream.next_execution = stream.next_execution.checked_add(1).ok_or_else(|| failure("execution_id_overflow"))?;
            stream.pending.push(execution.clone());
            stream.observer.generated(&execution)?;
        }
        Ok(())
    }

    pub(crate) fn respond_checked(&mut self, order: Order) -> Result<(), BacktestError> {
        if self.execution.is_none() {
            self.respond(order);
            return Ok(());
        }
        let stream = self.execution.as_mut().unwrap();
        stream.limits.admit_response(stream.next_response, self.to_local.len())?;
        if stream.pending.iter().any(|e| e.order_id != order.order_id)
        {
            return Err(failure("response_capacity_or_order"));
        }
        let envelope = ResponseEnvelope {
            response_id: stream.next_response,
            executions: stream.pending.clone(),
        };
        let arrival =
            order.exch_timestamp + self.order_latency.response(order.exch_timestamp, &order);
        let actual = self
            .to_local
            .append_envelope(order.clone(), arrival, Some(envelope.clone()));
        stream.next_response = stream.next_response.checked_add(1).ok_or_else(|| failure("response_id_overflow"))?;
        stream.pending.clear();
        stream.observer.registered(&order, &envelope, actual)
    }

    /// Returns the timestamp of the earliest order to be received by the exchange from the local.
    pub fn earliest_recv_order_timestamp(&self) -> Option<i64> {
        self.to_exch.earliest_timestamp()
    }

    /// Returns the timestamp of the earliest order sent from the exchange to the local.
    pub fn earliest_send_order_timestamp(&self) -> Option<i64> {
        self.to_local.earliest_timestamp()
    }

    /// Responds to the local with the order processed by the exchange.
    pub fn respond(&mut self, order: Order) {
        let local_recv_timestamp =
            order.exch_timestamp + self.order_latency.response(order.exch_timestamp, &order);
        self.to_local.append(order, local_recv_timestamp);
    }

    /// Receives the order request from the local, which is expected to be received at
    /// `receipt_timestamp`.
    pub fn receive(&mut self, receipt_timestamp: i64) -> Option<Order> {
        if let Some(timestamp) = self.to_exch.earliest_timestamp() {
            if timestamp == receipt_timestamp {
                self.to_exch.pop_front().map(|(order, _)| order)
            } else {
                assert!(timestamp > receipt_timestamp);
                None
            }
        } else {
            None
        }
    }
}

/// Provides a bidirectional order bus connecting the local to the exchange.
pub struct LocalToExch<LM> {
    to_exch: OrderBus,
    to_local: OrderBus,
    order_latency: LM,
}

impl<LM> LocalToExch<LM>
where
    LM: LatencyModel,
{
    /// Returns the timestamp of the earliest order to be received by the local from the exchange.
    pub fn earliest_recv_order_timestamp(&self) -> Option<i64> {
        self.to_local.earliest_timestamp()
    }

    /// Returns the timestamp of the earliest order sent from the local to the exchange.
    pub fn earliest_send_order_timestamp(&self) -> Option<i64> {
        self.to_exch.earliest_timestamp()
    }

    /// Sends the order request to the exchange.
    /// If it is rejected before reaching the matching engine (as reflected in the order latency
    /// information), `reject` is invoked and the rejection response is appended to the local order
    /// bus.
    pub fn request<F>(&mut self, mut order: Order, mut reject: F)
    where
        F: FnMut(&mut Order),
    {
        let order_entry_latency = self.order_latency.entry(order.local_timestamp, &order);
        // Negative latency indicates that the order is rejected for technical reasons, and its
        // value represents the latency that the local experiences when receiving the rejection
        // notification.
        if order_entry_latency < 0 {
            // Rejects the order.
            reject(&mut order);
            let rej_recv_timestamp = order.local_timestamp - order_entry_latency;
            self.to_local.append(order, rej_recv_timestamp);
        } else {
            let exch_recv_timestamp = order.local_timestamp + order_entry_latency;
            self.to_exch.append(order, exch_recv_timestamp);
        }
    }

    pub(crate) fn receive_envelope(
        &mut self,
        receipt_timestamp: i64,
    ) -> Option<(Order, Option<ResponseEnvelope>)> {
        if let Some(timestamp) = self.to_local.earliest_timestamp() {
            if timestamp == receipt_timestamp {
                return unsafe { &mut *self.to_local.order_list.get() }
                    .pop_front()
                    .map(|(o, _, e)| (o, e));
            }
            assert!(timestamp > receipt_timestamp);
        }
        None
    }

    /// Receives the order response from the exchange, which is expected to be received at
    /// `receipt_timestamp`.
    pub fn receive(&mut self, receipt_timestamp: i64) -> Option<Order> {
        if let Some(timestamp) = self.to_local.earliest_timestamp() {
            if timestamp == receipt_timestamp {
                self.to_local.pop_front().map(|(order, _)| order)
            } else {
                assert!(timestamp > receipt_timestamp);
                None
            }
        } else {
            None
        }
    }
}

/// Creates bidirectional order buses with the order latency model.
pub fn order_bus<LM>(order_latency: LM) -> (ExchToLocal<LM>, LocalToExch<LM>)
where
    LM: LatencyModel + Clone,
{
    let to_exch = OrderBus::new();
    let to_local = OrderBus::new();
    (
        ExchToLocal {
            to_exch: to_exch.clone(),
            to_local: to_local.clone(),
            order_latency: order_latency.clone(),
            execution: None,
        },
        LocalToExch {
            to_exch,
            to_local,
            order_latency,
        },
    )
}
