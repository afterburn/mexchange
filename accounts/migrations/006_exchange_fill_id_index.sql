-- Add unique index on exchange_fill_id for idempotency
-- This index is used for:
-- 1. Fast idempotency lookups (Trade::exists_by_fill_id, Trade::get_by_fill_id)
-- 2. ON CONFLICT handling in Trade::settle
CREATE UNIQUE INDEX IF NOT EXISTS idx_trades_exchange_fill_id ON trades(exchange_fill_id);
