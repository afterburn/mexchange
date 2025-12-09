import { memo } from 'react';
import { X } from 'lucide-react';
import type { Order } from '../types';

interface OpenOrdersProps {
  orders: Order[];
  onCancel: (orderId: number | string) => void;
}

const OrderRow = memo(function OrderRow({
  order,
  onCancel
}: {
  order: Order;
  onCancel: (id: number | string) => void;
}) {
  const filled = order.quantity - order.remainingQuantity;
  const fillPercent = (filled / order.quantity) * 100;

  return (
    <div className="flex items-center gap-2 px-3 py-1 hover:bg-white/5 text-xs transition-colors group">
      <span className={`w-10 font-medium ${order.side === 'Bid' ? 'text-green-400' : 'text-red-400'}`}>
        {order.side === 'Bid' ? 'BUY' : 'SELL'}
      </span>
      <span className="w-14 text-white/90">
        {order.price ? `€${order.price.toFixed(2)}` : 'MKT'}
      </span>
      <span className="w-20 text-white/70 text-right">
        {order.remainingQuantity.toFixed(2)} / {order.quantity.toFixed(2)}
      </span>
      <div className="w-12 h-1 bg-white/10 rounded-full overflow-hidden">
        <div
          className={`h-full ${order.side === 'Bid' ? 'bg-green-400/50' : 'bg-red-400/50'}`}
          style={{ width: `${fillPercent}%` }}
        />
      </div>
      <span className="w-12 text-white/50 text-right">{fillPercent.toFixed(0)}%</span>
      <button
        onClick={() => onCancel(order.id)}
        className="ml-auto opacity-0 group-hover:opacity-100 p-1 hover:bg-white/10 rounded transition-all"
        title="Cancel order"
      >
        <X size={12} className="text-white/60 hover:text-red-400" />
      </button>
    </div>
  );
});

const OpenOrders = memo(function OpenOrders({ orders, onCancel }: OpenOrdersProps) {
  const openOrders = orders.filter(order => order.remainingQuantity > 0);

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 text-[10px] text-white/40 px-3 py-1.5 border-b border-white/10 font-medium">
        <span className="w-10">Side</span>
        <span className="w-14">Price</span>
        <span className="w-20 text-right">Remaining</span>
        <span className="w-12">Fill</span>
        <span className="w-12 text-right">%</span>
        <span className="ml-auto text-white/30">{openOrders.length} open</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {openOrders.length === 0 ? (
          <div className="flex items-center justify-center h-full text-white/30 text-xs">
            No open orders
          </div>
        ) : (
          <div className="flex flex-col py-0.5">
            {openOrders.map((order) => (
              <OrderRow key={order.id} order={order} onCancel={onCancel} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
});

export default OpenOrders;
