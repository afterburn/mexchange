-- Make trade participant IDs nullable to support bot orders
-- Bot orders don't have a corresponding user or order in our system

-- Drop existing foreign key constraints
ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_buy_order_id_fkey;
ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_sell_order_id_fkey;
ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_buyer_id_fkey;
ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_seller_id_fkey;

-- Make columns nullable
ALTER TABLE trades ALTER COLUMN buy_order_id DROP NOT NULL;
ALTER TABLE trades ALTER COLUMN sell_order_id DROP NOT NULL;
ALTER TABLE trades ALTER COLUMN buyer_id DROP NOT NULL;
ALTER TABLE trades ALTER COLUMN seller_id DROP NOT NULL;

-- Re-add foreign key constraints but only validate when not null
ALTER TABLE trades ADD CONSTRAINT trades_buy_order_id_fkey
    FOREIGN KEY (buy_order_id) REFERENCES orders(id);
ALTER TABLE trades ADD CONSTRAINT trades_sell_order_id_fkey
    FOREIGN KEY (sell_order_id) REFERENCES orders(id);
ALTER TABLE trades ADD CONSTRAINT trades_buyer_id_fkey
    FOREIGN KEY (buyer_id) REFERENCES users(id);
ALTER TABLE trades ADD CONSTRAINT trades_seller_id_fkey
    FOREIGN KEY (seller_id) REFERENCES users(id);
