-- Add CHECK constraints for non-negative balances (defense in depth)
-- Use DO block for idempotency
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_available_non_negative'
    ) THEN
        ALTER TABLE balances ADD CONSTRAINT chk_available_non_negative CHECK (available >= 0);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_locked_non_negative'
    ) THEN
        ALTER TABLE balances ADD CONSTRAINT chk_locked_non_negative CHECK (locked >= 0);
    END IF;
END $$;

-- Make ledger table immutable (append-only)
CREATE OR REPLACE FUNCTION prevent_ledger_modification()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Ledger entries cannot be modified or deleted';
END;
$$ LANGUAGE plpgsql;

-- Drop trigger if exists (for idempotency)
DROP TRIGGER IF EXISTS ledger_immutable ON ledger;

CREATE TRIGGER ledger_immutable
    BEFORE UPDATE OR DELETE ON ledger
    FOR EACH ROW EXECUTE FUNCTION prevent_ledger_modification();
