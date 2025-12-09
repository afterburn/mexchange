-- Orders table: tracks all orders with their status
CREATE TABLE IF NOT EXISTS orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    exchange_order_id BIGINT,  -- ID from matching engine
    symbol VARCHAR(20) NOT NULL,  -- e.g., 'KCN/EUR'
    side VARCHAR(4) NOT NULL CHECK (side IN ('bid', 'ask')),
    order_type VARCHAR(10) NOT NULL CHECK (order_type IN ('limit', 'market')),
    price DECIMAL(20, 8),  -- NULL for market orders
    quantity DECIMAL(20, 8) NOT NULL,
    filled_quantity DECIMAL(20, 8) NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'open', 'partially_filled', 'filled', 'cancelled', 'rejected', 'expired')),
    lock_entry_id UUID,  -- Reference to the lock ledger entry
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_orders_user_id ON orders(user_id);
CREATE INDEX IF NOT EXISTS idx_orders_user_status ON orders(user_id, status);
CREATE INDEX IF NOT EXISTS idx_orders_exchange_order_id ON orders(exchange_order_id) WHERE exchange_order_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_orders_symbol_status ON orders(symbol, status);

-- Trades table: records all executed trades
CREATE TABLE IF NOT EXISTS trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol VARCHAR(20) NOT NULL,
    buy_order_id UUID NOT NULL REFERENCES orders(id),
    sell_order_id UUID NOT NULL REFERENCES orders(id),
    buyer_id UUID NOT NULL REFERENCES users(id),
    seller_id UUID NOT NULL REFERENCES users(id),
    price DECIMAL(20, 8) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    buyer_fee DECIMAL(20, 8) NOT NULL DEFAULT 0,
    seller_fee DECIMAL(20, 8) NOT NULL DEFAULT 0,
    exchange_fill_id VARCHAR(100),  -- Composite key from matching engine for idempotency
    settled_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_trades_exchange_fill ON trades(exchange_fill_id) WHERE exchange_fill_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trades_buyer ON trades(buyer_id, settled_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_seller ON trades(seller_id, settled_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol, settled_at DESC);

-- Faucet claims table for rate limiting
CREATE TABLE IF NOT EXISTS faucet_claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    asset VARCHAR(10) NOT NULL,
    amount DECIMAL(20, 8) NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_faucet_claims_user ON faucet_claims(user_id, asset, claimed_at DESC);
