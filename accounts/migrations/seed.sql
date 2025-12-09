-- Seed data for development/demo
-- Run with: psql -h localhost -U postgres -d accounts -f seed.sql

-- Sample user
INSERT INTO users (id, email) VALUES
    ('00000000-0000-0000-0000-000000000001', 'demo@mexchange.io')
ON CONFLICT (email) DO NOTHING;

-- Sample balances for demo user
INSERT INTO balances (user_id, asset, available, locked) VALUES
    ('00000000-0000-0000-0000-000000000001', 'EUR', 10000.00, 0),
    ('00000000-0000-0000-0000-000000000001', 'KCN', 5.0, 0)
ON CONFLICT (user_id, asset) DO NOTHING;

-- Generate OHLCV data for KCN/EUR (last 24 hours, 1-minute intervals)
-- Base price ~42 EUR with realistic fluctuations
INSERT INTO ohlcv (symbol, interval, open_time, open, high, low, close, volume, trade_count)
SELECT
    'KCN/EUR' as symbol,
    '1m' as interval,
    ts as open_time,
    -- Open: base price + trend + noise
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.5 as open,
    -- High: open + random upward movement
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.5 + random() * 0.3 as high,
    -- Low: open - random downward movement
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.5 - random() * 0.3 as low,
    -- Close: somewhere between high and low
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.6 as close,
    -- Volume: random between 0.1 and 2.0
    0.1 + random() * 1.9 as volume,
    -- Trade count: 1-20 trades per minute
    1 + floor(random() * 20)::int as trade_count
FROM generate_series(
    NOW() - INTERVAL '24 hours',
    NOW(),
    INTERVAL '1 minute'
) as ts
ON CONFLICT (symbol, interval, open_time) DO NOTHING;

-- Generate 5-minute OHLCV (aggregate from 1m data would be better, but this is seed data)
INSERT INTO ohlcv (symbol, interval, open_time, open, high, low, close, volume, trade_count)
SELECT
    'KCN/EUR' as symbol,
    '5m' as interval,
    ts as open_time,
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.8 as open,
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.8 + random() * 0.5 as high,
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.8 - random() * 0.5 as low,
    42 + (2 * sin(EXTRACT(EPOCH FROM ts) / 3600)) + (random() - 0.5) * 0.9 as close,
    0.5 + random() * 8 as volume,
    5 + floor(random() * 80)::int as trade_count
FROM generate_series(
    NOW() - INTERVAL '7 days',
    NOW(),
    INTERVAL '5 minutes'
) as ts
ON CONFLICT (symbol, interval, open_time) DO NOTHING;

-- Generate 1-hour OHLCV
INSERT INTO ohlcv (symbol, interval, open_time, open, high, low, close, volume, trade_count)
SELECT
    'KCN/EUR' as symbol,
    '1h' as interval,
    ts as open_time,
    42 + (3 * sin(EXTRACT(EPOCH FROM ts) / 43200)) + (random() - 0.5) * 1.5 as open,
    42 + (3 * sin(EXTRACT(EPOCH FROM ts) / 43200)) + (random() - 0.5) * 1.5 + random() * 1.0 as high,
    42 + (3 * sin(EXTRACT(EPOCH FROM ts) / 43200)) + (random() - 0.5) * 1.5 - random() * 1.0 as low,
    42 + (3 * sin(EXTRACT(EPOCH FROM ts) / 43200)) + (random() - 0.5) * 1.8 as close,
    5 + random() * 50 as volume,
    50 + floor(random() * 500)::int as trade_count
FROM generate_series(
    NOW() - INTERVAL '30 days',
    NOW(),
    INTERVAL '1 hour'
) as ts
ON CONFLICT (symbol, interval, open_time) DO NOTHING;
