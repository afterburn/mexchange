-- Ledger: append-only transaction log (source of truth for balances)
CREATE TABLE IF NOT EXISTS ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    asset VARCHAR(10) NOT NULL,
    amount DECIMAL(20, 8) NOT NULL,  -- positive = credit, negative = debit
    balance_after DECIMAL(20, 8) NOT NULL,  -- running balance for fast queries
    entry_type VARCHAR(20) NOT NULL,  -- 'deposit', 'withdrawal', 'trade', 'fee', 'lock', 'unlock'
    reference_id UUID,  -- optional: links to order_id, trade_id, etc.
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fetching user's ledger history
CREATE INDEX IF NOT EXISTS idx_ledger_user_asset ON ledger(user_id, asset, created_at DESC);

-- Index for reconciliation queries
CREATE INDEX IF NOT EXISTS idx_ledger_user_asset_type ON ledger(user_id, asset, entry_type);

-- Index for reference lookups (e.g., find all entries for an order)
CREATE INDEX IF NOT EXISTS idx_ledger_reference ON ledger(reference_id) WHERE reference_id IS NOT NULL;
