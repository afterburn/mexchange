export type Side = 'Bid' | 'Ask';
export type OrderType = 'Limit' | 'Market';

export interface Order {
  id: number | string;
  side: Side;
  orderType: OrderType;
  price: number | null;
  quantity: number;
  remainingQuantity: number;
}

export interface OrderBookLevel {
  price: number;
  quantity: number;
  total: number;
  // Pre-formatted strings for HFT performance
  priceStr: string;
  quantityStr: string;
  totalStr: string;
}

export interface Trade {
  id: number;
  price: number;
  quantity: number;
  side: Side;
  timestamp: number;
  // Pre-formatted strings for HFT performance
  priceStr: string;
  quantityStr: string;
  timeStr: string;
}

export interface MarketStats {
  symbol: string;
  lastPrice: number;
  priceChange24h: number;
  priceChangePercent24h: number;
  high24h: number;
  low24h: number;
  volume24h: number;
  bestBid: number | null;
  bestAsk: number | null;
  spread: number | null;
}

export interface OrderBook {
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}


