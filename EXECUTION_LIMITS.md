# Explicit execution delivery budgets

Opt-in `with_execution_observer_limits` changes only capacity admission and checked ID increments. The old `with_execution_observer` remains 16 cumulative executions / 32 cumulative responses. Matching, queue model, scheduler, latency, fee, E generation/Q registration/R consumer semantics are unchanged.

`ExecutionLimits` separates cumulative IDs from pending executions (maximum 16 per response) and queued unreceived responses (maximum 32; the research profile selects 1). IDs are monotonic, never reset/reused; limits reserve the next-ID sentinel and reject overflow. The existing OrderBus and consumer remain the implementation. Total queue safety for the old default is unchanged: at most its 32 cumulative responses could previously be outstanding.

The codex-cloud research-order-budget-v1 profile selects executions384/responses24/pending executions16/queued responses1. Its order admission reserves a Sell and notifications before opening inventory. Application tests distinguish native repeated FOK executions from direct 16/32/overflow/queue boundary admission checks. This commit is source only until the externally recorded finite build/suite result; no market execution is authorized by this document.
