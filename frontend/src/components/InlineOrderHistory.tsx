import { memo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { Order } from '../api/accounts';

interface InlineOrderHistoryProps {
  orders: Order[];
  isLoading?: boolean;
}

const OrderRow = memo(function OrderRow({ order }: { order: Order }) {
  const navigate = useNavigate();
  const isBuy = order.side === 'bid';
  const date = new Date(order.created_at);
  const filled = parseFloat(order.filled_quantity);
  const qty = parseFloat(order.quantity);
  const fillPercent = qty > 0 ? (filled / qty * 100).toFixed(0) : '0';

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'filled': return 'text-green-400';
      case 'partially_filled': return 'text-yellow-400';
      case 'cancelled': return 'text-red-400';
      default: return 'text-white/40';
    }
  };

  const formatStatus = (status: string) => {
    switch (status) {
      case 'filled': return 'Filled';
      case 'partially_filled': return 'Partial';
      case 'cancelled': return 'Cancelled';
      case 'pending': return 'Pending';
      default: return status;
    }
  };

  return (
    <div
      onClick={() => navigate(`/orders/${order.id}`)}
      className="flex items-center gap-2 px-3 py-1.5 hover:bg-white/5 text-xs transition-colors cursor-pointer"
    >
      <span className={`w-10 font-medium ${isBuy ? 'text-green-400' : 'text-red-400'}`}>
        {isBuy ? 'BUY' : 'SELL'}
      </span>
      <span className="w-16 text-white/90 text-right tabular-nums">
        {order.price ? parseFloat(order.price).toFixed(2) : 'MKT'}
      </span>
      <span className="w-20 text-white/70 text-right tabular-nums">
        {qty.toFixed(4)}
      </span>
      <span className="w-16 text-white/50 text-right tabular-nums">
        {fillPercent}%
      </span>
      <span className={`w-14 ${getStatusColor(order.status)}`}>
        {formatStatus(order.status)}
      </span>
      <span className="flex-1 text-white/40 text-right">
        {date.toLocaleDateString()} {date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
      </span>
    </div>
  );
});

const InlineOrderHistory = memo(function InlineOrderHistory({ orders, isLoading }: InlineOrderHistoryProps) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 text-[10px] text-white/40 px-3 py-1.5 border-b border-white/10 font-medium">
        <span className="w-10">Side</span>
        <span className="w-16 text-right">Price</span>
        <span className="w-20 text-right">Qty</span>
        <span className="w-16 text-right">Filled</span>
        <span className="w-14">Status</span>
        <span className="flex-1 text-right">Time</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-full text-white/30 text-xs">
            Loading...
          </div>
        ) : orders.length === 0 ? (
          <div className="flex items-center justify-center h-full text-white/30 text-xs">
            No order history
          </div>
        ) : (
          <div className="flex flex-col py-0.5">
            {orders.map((order) => (
              <OrderRow key={order.id} order={order} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
});

export default InlineOrderHistory;
