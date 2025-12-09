import { useState, useEffect, memo } from 'react';
import { Link } from 'react-router-dom';
import type { Side, OrderType } from '../types';

interface TradeFormProps {
  onPlaceOrder: (side: Side, orderType: OrderType, price: number | null, quantity: number | null, quoteAmount?: number) => void;
  bestBid: number | null;
  bestAsk: number | null;
  balance: { eur: number; kcn: number };
  isLoggedIn?: boolean;
}

const TradeForm = memo(function TradeForm({ onPlaceOrder, bestBid, bestAsk, balance, isLoggedIn = false }: TradeFormProps) {
  const [side, setSide] = useState<Side>('Bid');
  const [orderType, setOrderType] = useState<OrderType>('Limit');
  const [price, setPrice] = useState<string>('');
  const [quantity, setQuantity] = useState<string>('');
  const [percentage, setPercentage] = useState<number>(0);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const priceNum = orderType === 'Limit' ? parseFloat(price) : null;
    const quantityNum = parseFloat(quantity);

    if (quantityNum > 0 && (orderType === 'Market' || (priceNum && priceNum > 0))) {
      // For market buy orders, use quote currency order (send EUR amount, not quantity)
      // This avoids rounding issues when trying to spend 100% of balance
      if (orderType === 'Market' && side === 'Bid' && percentage > 0) {
        const quoteAmount = balance.eur * percentage / 100;
        onPlaceOrder(side, orderType, priceNum, null, quoteAmount);
      } else {
        onPlaceOrder(side, orderType, priceNum, quantityNum);
      }
      setPrice('');
      setQuantity('');
      setPercentage(0);
    }
  };

  const calculateQuantityFromPercentage = (pct: number) => {
    if (pct === 0) return '';
    if (side === 'Bid') {
      const maxEur = balance.eur;
      // For market orders, use bestAsk * 1.05 (slippage buffer) since that's what gets locked
      // For limit orders, use entered price or bestAsk
      const priceNum = orderType === 'Limit'
        ? (parseFloat(price) || bestAsk || 0)
        : ((bestAsk || 0) * 1.05); // 5% slippage buffer for market orders
      if (priceNum > 0) {
        const qty = (maxEur * pct / 100) / priceNum;
        return qty > 0 ? qty.toFixed(8) : '';
      }
      return '';
    } else {
      const qty = balance.kcn * pct / 100;
      return qty > 0 ? qty.toFixed(8) : '';
    }
  };

  const handlePercentageClick = (pct: number) => {
    setPercentage(pct);
    setQuantity(calculateQuantityFromPercentage(pct));
  };

  // Auto-update quantity when orderbook prices change (only if percentage is selected)
  useEffect(() => {
    if (percentage > 0) {
      setQuantity(calculateQuantityFromPercentage(percentage));
    }
  }, [bestAsk, bestBid]);

  const total = orderType === 'Limit' && price ? parseFloat(price) * parseFloat(quantity || '0') : 0;

  return (
    <div className="flex flex-col shrink-0">
      <div className="flex border-b border-white/10">
        <button
          onClick={() => setSide('Bid')}
          className={`flex-1 py-2 text-xs font-medium transition-colors ${
            side === 'Bid'
              ? 'text-green-400 border-b-2 border-green-400'
              : 'text-white/40 hover:text-white/60'
          }`}
        >
          Buy
        </button>
        <button
          onClick={() => setSide('Ask')}
          className={`flex-1 py-2 text-xs font-medium transition-colors ${
            side === 'Ask'
              ? 'text-red-400 border-b-2 border-red-400'
              : 'text-white/40 hover:text-white/60'
          }`}
        >
          Sell
        </button>
      </div>

      <form onSubmit={handleSubmit} className="flex flex-col p-4 space-y-3">
        <div>
          <div className="flex justify-between mb-1.5">
            <label className="text-xs text-white/60 font-medium">Order Type</label>
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setOrderType('Limit')}
              className={`flex-1 py-2 text-xs rounded transition-colors ${
                orderType === 'Limit'
                  ? 'bg-white text-black border border-white'
                  : 'bg-white/5 text-white/60 border border-white/10 hover:bg-white/10'
              }`}
            >
              Limit
            </button>
            <button
              type="button"
              onClick={() => setOrderType('Market')}
              className={`flex-1 py-2 text-xs rounded transition-colors ${
                orderType === 'Market'
                  ? 'bg-white text-black border border-white'
                  : 'bg-white/5 text-white/60 border border-white/10 hover:bg-white/10'
              }`}
            >
              Market
            </button>
          </div>
        </div>

        {orderType === 'Limit' && (
          <div>
            <div className="flex justify-between mb-1.5">
              <label className="text-xs text-white/60 font-medium">Price</label>
              <span className="text-xs text-white/40">EUR</span>
            </div>
            <div className="relative">
              <input
                type="number"
                step="0.01"
                value={price}
                onChange={(e) => setPrice(e.target.value)}
                placeholder="0.00"
                className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white placeholder-white/30 focus:outline-none focus:border-white/30 transition-colors"
              />
              <div className="absolute right-2 top-1.5 flex gap-1.5">
                <button
                  type="button"
                  onClick={() => bestBid && setPrice(bestBid.toFixed(2))}
                  className="text-[10px] text-white/60 hover:text-white transition-colors"
                >
                  Bid
                </button>
                <button
                  type="button"
                  onClick={() => bestAsk && setPrice(bestAsk.toFixed(2))}
                  className="text-[10px] text-white/60 hover:text-white transition-colors"
                >
                  Ask
                </button>
              </div>
            </div>
          </div>
        )}

        <div>
          <div className="flex justify-between mb-1.5">
            <label className="text-xs text-white/60 font-medium">Quantity</label>
            <span className="text-xs text-white/40">KCN</span>
          </div>
          <input
            type="number"
            step="0.00000001"
            value={quantity}
            onChange={(e) => { setQuantity(e.target.value); setPercentage(0); }}
            placeholder="0.00"
            className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white placeholder-white/30 focus:outline-none focus:border-white/30 transition-colors"
          />
          <div className="flex gap-1.5 mt-2">
            {[25, 50, 75, 100].map((pct) => (
              <button
                key={pct}
                type="button"
                onClick={() => handlePercentageClick(pct)}
                className={`flex-1 py-1 text-[10px] rounded transition-colors ${
                  percentage === pct
                    ? 'bg-white text-black border border-white'
                    : 'bg-white/5 text-white/60 border border-white/10 hover:bg-white/10'
                }`}
              >
                {pct}%
              </button>
            ))}
          </div>
        </div>

        {orderType === 'Limit' && price && quantity && (
          <div className="pt-2 border-t border-white/10">
            <div className="flex justify-between text-xs">
              <span className="text-white/60">Total</span>
              <span className="text-white font-medium">€{total.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}</span>
            </div>
          </div>
        )}

        {isLoggedIn ? (
          <button
            type="submit"
            className={`w-full py-2.5 rounded text-sm font-medium transition-colors ${
              side === 'Bid'
                ? 'bg-green-500 text-white hover:bg-green-600'
                : 'bg-red-500 text-white hover:bg-red-600'
            }`}
          >
            {side === 'Bid' ? 'Buy KCN' : 'Sell KCN'}
          </button>
        ) : (
          <Link
            to="/signin"
            className="w-full py-2.5 rounded text-sm font-medium bg-white text-black hover:bg-white/90 transition-colors text-center block"
          >
            Sign in to trade
          </Link>
        )}

        <div className="text-[10px] text-white/40 pt-2 border-t border-white/10 space-y-1">
          <div className="flex justify-between">
            <span>Available</span>
            <span className="text-white/60">{side === 'Bid' ? `${balance.eur.toFixed(2)} EUR` : `${balance.kcn.toFixed(2)} KCN`}</span>
          </div>
          {orderType === 'Market' && side === 'Bid' && bestAsk && (
            <div className="flex justify-between">
              <span>Slippage</span>
              <span className="text-white/60">5% (max €{(bestAsk * 1.05).toFixed(2)}/KCN)</span>
            </div>
          )}
        </div>
      </form>
    </div>
  );
});

export default TradeForm;

