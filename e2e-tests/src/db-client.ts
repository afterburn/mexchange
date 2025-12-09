/**
 * Database Client for E2E Tests
 * Direct database access for verifying state
 */

import pg from 'pg';
const { Pool } = pg;

const DATABASE_URL = process.env.DATABASE_URL || 'postgres://postgres:postgres@localhost:5433/accounts';

export interface DbUser {
  id: string;
  email: string;
  created_at: Date;
}

export interface DbOtp {
  id: string;
  email: string;
  code: string;
  created_at: Date;
  expires_at: Date;
  used: boolean;
}

export interface DbOrder {
  id: string;
  user_id: string;
  symbol: string;
  side: string;
  order_type: string;
  price: string | null;
  quantity: string;
  filled_quantity: string;
  status: string;
  created_at: Date;
  updated_at: Date;
}

export interface DbTrade {
  id: string;
  symbol: string;
  buy_order_id: string | null;
  sell_order_id: string | null;
  buyer_id: string | null;
  seller_id: string | null;
  price: string;
  quantity: string;
  buyer_fee: string;
  seller_fee: string;
  exchange_fill_id: string | null;
  settled_at: Date;
}

export interface DbLedgerEntry {
  id: string;
  user_id: string;
  asset: string;
  amount: string;
  balance_after: string;
  entry_type: string;
  reference_id: string | null;  // order_id for order-related entries
  description: string | null;
  created_at: Date;
}

export interface DbBalance {
  user_id: string;
  asset: string;
  available: string;
  locked: string;
  updated_at: Date;
}

export class DbClient {
  private pool: pg.Pool;

  constructor() {
    this.pool = new Pool({
      connectionString: DATABASE_URL,
    });
  }

  async connect(): Promise<void> {
    // Test connection
    const client = await this.pool.connect();
    client.release();
  }

  async disconnect(): Promise<void> {
    await this.pool.end();
  }

  /**
   * Get OTP code for email (for test authentication)
   */
  async getLatestOtp(email: string): Promise<DbOtp | null> {
    const result = await this.pool.query<DbOtp>(
      `SELECT id, email, code, created_at, expires_at, used
       FROM otps
       WHERE email = $1 AND used = false AND expires_at > NOW()
       ORDER BY created_at DESC
       LIMIT 1`,
      [email.toLowerCase()]
    );
    return result.rows[0] || null;
  }

  /**
   * Get user by email
   */
  async getUserByEmail(email: string): Promise<DbUser | null> {
    const result = await this.pool.query<DbUser>(
      `SELECT id, email, created_at FROM users WHERE email = $1`,
      [email.toLowerCase()]
    );
    return result.rows[0] || null;
  }

  /**
   * Get user by ID
   */
  async getUserById(userId: string): Promise<DbUser | null> {
    const result = await this.pool.query<DbUser>(
      `SELECT id, email, created_at FROM users WHERE id = $1`,
      [userId]
    );
    return result.rows[0] || null;
  }

  /**
   * Get user's orders
   */
  async getOrdersForUser(userId: string): Promise<DbOrder[]> {
    const result = await this.pool.query<DbOrder>(
      `SELECT id, user_id, symbol, side, order_type, price, quantity,
              filled_quantity, status, created_at, updated_at
       FROM orders
       WHERE user_id = $1
       ORDER BY created_at DESC`,
      [userId]
    );
    return result.rows;
  }

  /**
   * Get order by ID
   */
  async getOrderById(orderId: string): Promise<DbOrder | null> {
    const result = await this.pool.query<DbOrder>(
      `SELECT id, user_id, symbol, side, order_type, price, quantity,
              filled_quantity, status, created_at, updated_at
       FROM orders
       WHERE id = $1`,
      [orderId]
    );
    return result.rows[0] || null;
  }

  /**
   * Get trades for user (as buyer or seller)
   */
  async getTradesForUser(userId: string): Promise<DbTrade[]> {
    const result = await this.pool.query<DbTrade>(
      `SELECT id, symbol, buy_order_id, sell_order_id, buyer_id, seller_id,
              price, quantity, buyer_fee, seller_fee, exchange_fill_id, settled_at
       FROM trades
       WHERE buyer_id = $1 OR seller_id = $1
       ORDER BY settled_at DESC`,
      [userId]
    );
    return result.rows;
  }

  /**
   * Get all trades for an order
   */
  async getTradesForOrder(orderId: string): Promise<DbTrade[]> {
    const result = await this.pool.query<DbTrade>(
      `SELECT id, symbol, buy_order_id, sell_order_id, buyer_id, seller_id,
              price, quantity, buyer_fee, seller_fee, exchange_fill_id, settled_at
       FROM trades
       WHERE buy_order_id = $1 OR sell_order_id = $1
       ORDER BY settled_at DESC`,
      [orderId]
    );
    return result.rows;
  }

  /**
   * Get ledger entries for user
   */
  async getLedgerEntriesForUser(userId: string, asset?: string): Promise<DbLedgerEntry[]> {
    let query = `SELECT id, user_id, asset, amount, balance_after, entry_type,
                        reference_id, description, created_at
                 FROM ledger
                 WHERE user_id = $1`;
    const params: string[] = [userId];

    if (asset) {
      query += ` AND asset = $2`;
      params.push(asset);
    }

    query += ` ORDER BY created_at DESC`;

    const result = await this.pool.query<DbLedgerEntry>(query, params);
    return result.rows;
  }

  /**
   * Get ledger entries for an order (by reference_id)
   */
  async getLedgerEntriesForOrder(orderId: string): Promise<DbLedgerEntry[]> {
    const result = await this.pool.query<DbLedgerEntry>(
      `SELECT id, user_id, asset, amount, balance_after, entry_type,
              reference_id, description, created_at
       FROM ledger
       WHERE reference_id = $1
       ORDER BY created_at ASC`,
      [orderId]
    );
    return result.rows;
  }

  /**
   * Get user's balance for an asset
   */
  async getBalance(userId: string, asset: string): Promise<DbBalance | null> {
    const result = await this.pool.query<DbBalance>(
      `SELECT user_id, asset, available, locked, updated_at
       FROM balances
       WHERE user_id = $1 AND asset = $2`,
      [userId, asset]
    );
    return result.rows[0] || null;
  }

  /**
   * Get all balances for user
   */
  async getBalancesForUser(userId: string): Promise<DbBalance[]> {
    const result = await this.pool.query<DbBalance>(
      `SELECT user_id, asset, available, locked, updated_at
       FROM balances
       WHERE user_id = $1`,
      [userId]
    );
    return result.rows;
  }

  /**
   * Count ledger entries of specific type for order
   */
  async countLedgerEntriesByType(orderId: string, entryType: string): Promise<number> {
    const result = await this.pool.query<{ count: string }>(
      `SELECT COUNT(*) as count FROM ledger
       WHERE reference_id = $1 AND entry_type = $2`,
      [orderId, entryType]
    );
    return parseInt(result.rows[0]?.count || '0', 10);
  }

  /**
   * Delete test user and all related data (for cleanup)
   * Note: Ledger entries cannot be deleted due to immutability trigger,
   * so we delete the user (which cascades) instead
   */
  async deleteTestUser(email: string): Promise<void> {
    const user = await this.getUserByEmail(email);
    if (!user) return;

    try {
      // Delete in order of dependencies - ledger has ON DELETE CASCADE from users
      await this.pool.query(`DELETE FROM faucet_claims WHERE user_id = $1`, [user.id]);
      await this.pool.query(`DELETE FROM refresh_tokens WHERE user_id = $1`, [user.id]);
      // Set trades to null for this user (can't delete as other user might reference)
      await this.pool.query(
        `UPDATE trades SET buyer_id = NULL WHERE buyer_id = $1`,
        [user.id]
      );
      await this.pool.query(
        `UPDATE trades SET seller_id = NULL WHERE seller_id = $1`,
        [user.id]
      );
      await this.pool.query(`DELETE FROM orders WHERE user_id = $1`, [user.id]);
      await this.pool.query(`DELETE FROM balances WHERE user_id = $1`, [user.id]);
      // Deleting user will cascade to ledger entries
      await this.pool.query(`DELETE FROM users WHERE id = $1`, [user.id]);
      await this.pool.query(`DELETE FROM otps WHERE email = $1`, [email.toLowerCase()]);
    } catch (e) {
      // Ignore cleanup errors - test data will accumulate but tests should still work
      console.warn(`Cleanup warning for ${email}: ${e}`);
    }
  }

  /**
   * Raw query for custom checks
   */
  async query<T>(sql: string, params?: unknown[]): Promise<T[]> {
    const result = await this.pool.query<T>(sql, params);
    return result.rows;
  }
}
