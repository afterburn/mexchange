-- Remove exchange_order_id column from orders table
-- This column is no longer needed since we use UUID as the single order identifier throughout the system

-- Drop the index first
DROP INDEX IF EXISTS idx_orders_exchange_order_id;

-- Remove the column
ALTER TABLE orders DROP COLUMN IF EXISTS exchange_order_id;
