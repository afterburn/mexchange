import { memo } from 'react';
import type { Trade } from '../api/accounts';

interface TradeHistoryProps {
  trades: Trade[];
  isLoading?: boolean;
}

const TradeRow = memo(function TradeRow({ trade }: { trade: Trade }) {
  const isBuy = trade.side === 'buy';
  const date = new Date(trade.settled_at);

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 hover:bg-white/5 text-xs transition-colors">
      <span className={`w-10 font-medium ${isBuy ? 'text-green-400' : 'text-red-400'}`}>
        {isBuy ? 'BUY' : 'SELL'}
      </span>
      <span className="w-16 text-white/90 text-right tabular-nums">
        {parseFloat(trade.price).toFixed(2)}
      </span>
      <span className="w-20 text-white/70 text-right tabular-nums">
        {parseFloat(trade.quantity).toFixed(4)}
      </span>
      <span className="w-20 text-white/90 text-right tabular-nums">
        {parseFloat(trade.total).toFixed(2)}
      </span>
      <span className="flex-1 text-white/40 text-right">
        {date.toLocaleDateString()} {date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
      </span>
    </div>
  );
});

const TradeHistory = memo(function TradeHistory({ trades, isLoading }: TradeHistoryProps) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 text-[10px] text-white/40 px-3 py-1.5 border-b border-white/10 font-medium">
        <span className="w-10">Side</span>
        <span className="w-16 text-right">Price</span>
        <span className="w-20 text-right">Qty</span>
        <span className="w-20 text-right">Total</span>
        <span className="flex-1 text-right">Time</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-full text-white/30 text-xs">
            Loading...
          </div>
        ) : trades.length === 0 ? (
          <div className="flex items-center justify-center h-full text-white/30 text-xs">
            No order history
          </div>
        ) : (
          <div className="flex flex-col py-0.5">
            {trades.map((trade) => (
              <TradeRow key={trade.id} trade={trade} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
});

export default TradeHistory;
