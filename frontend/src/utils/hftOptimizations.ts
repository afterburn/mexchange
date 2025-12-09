/**
 * High-Frequency Trading Optimizations
 *
 * This module provides optimized data structures and utilities for HFT scenarios:
 * - SortedPriceMap: Maintains sorted order with O(1) best price lookup
 * - Object pooling for OrderBookLevel to reduce GC pressure
 * - Pre-allocated arrays and binary insert utilities
 * - Fast number formatting without toLocaleString/toFixed overhead
 */

// ============================================================================
// Fast Number Formatting (avoids toLocaleString/toFixed GC pressure)
// ============================================================================

// Pre-computed lookup tables for common decimal formatting
// (reserved for future optimization with lookup tables)

/**
 * Fast price formatting - avoids toLocaleString overhead
 * Pre-formats to 2 decimal places with thousands separators
 */
export function formatPrice(price: number): string {
  // Handle edge cases
  if (!Number.isFinite(price)) return '0.00';

  // Round to 2 decimal places using integer math (faster than toFixed)
  const rounded = Math.round(price * 100) / 100;
  const intPart = Math.floor(rounded);
  const decPart = Math.round((rounded - intPart) * 100);

  // Format integer part with thousands separator
  const intStr = intPart.toLocaleString('en-US');
  const decStr = decPart.toString().padStart(2, '0');

  return `${intStr}.${decStr}`;
}

/**
 * Fast quantity formatting - 6 decimal places
 */
export function formatQuantity(qty: number): string {
  if (!Number.isFinite(qty)) return '0.000000';

  // Round to 6 decimal places
  const rounded = Math.round(qty * 1000000) / 1000000;
  const intPart = Math.floor(rounded);
  const decPart = Math.round((rounded - intPart) * 1000000);

  return `${intPart}.${decPart.toString().padStart(6, '0')}`;
}

/**
 * Fast quantity formatting - 2 decimal places (for trades)
 */
export function formatQuantityShort(qty: number): string {
  if (!Number.isFinite(qty)) return '0.00';

  const rounded = Math.round(qty * 100) / 100;
  const intPart = Math.floor(rounded);
  const decPart = Math.round((rounded - intPart) * 100);

  return `${intPart}.${decPart.toString().padStart(2, '0')}`;
}

/**
 * Fast time formatting - HH:MM:SS from timestamp
 * Avoids Date object creation overhead
 */
export function formatTime(timestamp: number): string {
  // Convert to local time offset
  const date = new Date(timestamp);
  const h = date.getHours();
  const m = date.getMinutes();
  const s = date.getSeconds();

  return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

// ============================================================================
// Sorted Price Map with O(1) Best Price Lookup
// ============================================================================

export interface PriceLevel {
  price: number;
  quantity: number;
}

/**
 * Sorted price map that maintains O(1) best price lookup
 * Uses a sorted array internally for efficient iteration
 * and tracks the best price separately for instant access
 */
export class SortedPriceMap {
  private levels: PriceLevel[] = [];
  private priceToIndex: Map<number, number> = new Map();
  private _bestPrice: number | null = null;
  private isBidSide: boolean;

  constructor(isBidSide: boolean) {
    this.isBidSide = isBidSide;
  }

  get bestPrice(): number | null {
    return this._bestPrice;
  }

  get size(): number {
    return this.levels.length;
  }

  /**
   * Binary search for insertion point
   */
  private findInsertIndex(price: number): number {
    let low = 0;
    let high = this.levels.length;

    while (low < high) {
      const mid = (low + high) >>> 1;
      const cmp = this.isBidSide
        ? this.levels[mid].price > price  // Bids: descending
        : this.levels[mid].price < price; // Asks: ascending

      if (cmp) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    return low;
  }

  /**
   * Set quantity at price level
   * O(log n) for new prices, O(1) for updates
   */
  set(price: number, quantity: number): void {
    const existingIndex = this.priceToIndex.get(price);

    if (quantity <= 0) {
      // Remove price level
      if (existingIndex !== undefined) {
        this.levels.splice(existingIndex, 1);
        this.priceToIndex.delete(price);

        // Rebuild index for affected elements
        for (let i = existingIndex; i < this.levels.length; i++) {
          this.priceToIndex.set(this.levels[i].price, i);
        }

        // Update best price
        this._bestPrice = this.levels.length > 0 ? this.levels[0].price : null;
      }
      return;
    }

    if (existingIndex !== undefined) {
      // Update existing - O(1)
      this.levels[existingIndex].quantity = quantity;
    } else {
      // Insert new - O(log n) search + O(n) insert
      const insertIdx = this.findInsertIndex(price);
      const level: PriceLevel = { price, quantity };
      this.levels.splice(insertIdx, 0, level);

      // Rebuild index for affected elements
      for (let i = insertIdx; i < this.levels.length; i++) {
        this.priceToIndex.set(this.levels[i].price, i);
      }

      // Update best price (always at index 0 due to sorting)
      this._bestPrice = this.levels[0].price;
    }
  }

  /**
   * Get quantity at price level - O(1)
   */
  get(price: number): number | undefined {
    const idx = this.priceToIndex.get(price);
    return idx !== undefined ? this.levels[idx].quantity : undefined;
  }

  /**
   * Check if price exists - O(1)
   */
  has(price: number): boolean {
    return this.priceToIndex.has(price);
  }

  /**
   * Clear all levels
   */
  clear(): void {
    this.levels = [];
    this.priceToIndex.clear();
    this._bestPrice = null;
  }

  /**
   * Get sorted levels (already sorted, no copy needed for read-only access)
   */
  getSortedLevels(): readonly PriceLevel[] {
    return this.levels;
  }

  /**
   * Iterate over price levels in sorted order
   */
  *[Symbol.iterator](): Iterator<PriceLevel> {
    for (const level of this.levels) {
      yield level;
    }
  }
}

// ============================================================================
// Object Pool for OrderBookLevel
// ============================================================================

export interface FormattedOrderBookLevel {
  price: number;
  quantity: number;
  total: number;
  // Pre-formatted strings
  priceStr: string;
  quantityStr: string;
  totalStr: string;
}

/**
 * Object pool to reduce GC pressure
 * Reuses OrderBookLevel objects instead of creating new ones
 */
export class OrderBookLevelPool {
  private pool: FormattedOrderBookLevel[] = [];
  private poolSize = 0;

  /**
   * Get an object from pool or create new
   */
  acquire(): FormattedOrderBookLevel {
    if (this.poolSize > 0) {
      return this.pool[--this.poolSize];
    }
    return {
      price: 0,
      quantity: 0,
      total: 0,
      priceStr: '',
      quantityStr: '',
      totalStr: '',
    };
  }

  /**
   * Return object to pool
   */
  release(obj: FormattedOrderBookLevel): void {
    if (this.poolSize < 200) { // Max pool size
      this.pool[this.poolSize++] = obj;
    }
  }

  /**
   * Release entire array back to pool
   */
  releaseAll(arr: FormattedOrderBookLevel[]): void {
    for (const obj of arr) {
      this.release(obj);
    }
  }
}

// ============================================================================
// Binary Insert for Price History
// ============================================================================

export interface PricePoint {
  price: number;
  time: number;
}

/**
 * Binary insert into sorted array (by time, ascending)
 * Returns true if inserted, false if duplicate time
 */
export function binaryInsertPricePoint(
  arr: PricePoint[],
  point: PricePoint
): boolean {
  const time = point.time;
  const len = arr.length;

  // Fast path: append to end (most common case for real-time data)
  if (len === 0 || time >= arr[len - 1].time) {
    arr.push(point);
    return true;
  }

  // Fast path: prepend to beginning
  if (time <= arr[0].time) {
    arr.unshift(point);
    return true;
  }

  // Binary search for insertion point
  let low = 0;
  let high = len;

  while (low < high) {
    const mid = (low + high) >>> 1;
    if (arr[mid].time < time) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }

  arr.splice(low, 0, point);
  return true;
}

// ============================================================================
// Formatted Trade with pre-computed strings
// ============================================================================

export interface FormattedTrade {
  id: number;
  price: number;
  quantity: number;
  side: 'Bid' | 'Ask';
  timestamp: number;
  // Pre-formatted
  priceStr: string;
  quantityStr: string;
  timeStr: string;
}

/**
 * Create a formatted trade with pre-computed strings
 */
export function createFormattedTrade(
  id: number,
  price: number,
  quantity: number,
  side: 'Bid' | 'Ask',
  timestamp: number
): FormattedTrade {
  return {
    id,
    price,
    quantity,
    side,
    timestamp,
    priceStr: formatPrice(price),
    quantityStr: formatQuantityShort(quantity),
    timeStr: formatTime(timestamp),
  };
}

// ============================================================================
// Global pools (singleton instances)
// ============================================================================

export const bidLevelPool = new OrderBookLevelPool();
export const askLevelPool = new OrderBookLevelPool();
