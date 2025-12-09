import { useEffect, useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { accountsAPI, type Order } from '../api/accounts';

interface OrderHistoryProps {
  pageSize?: number;
  showPagination?: boolean;
  compact?: boolean;
}

export default function OrderHistory({
  pageSize = 10,
  showPagination = true,
  compact = false
}: OrderHistoryProps) {
  const navigate = useNavigate();
  const [orders, setOrders] = useState<Order[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [isLoading, setIsLoading] = useState(true);

  const totalPages = Math.ceil(total / pageSize);

  const fetchOrders = useCallback(async (pageNum: number) => {
    setIsLoading(true);
    try {
      const result = await accountsAPI.getOrders(pageSize, pageNum * pageSize);
      setOrders(result.orders);
      setTotal(result.total);
    } catch (e) {
      console.error('Failed to fetch orders:', e);
    } finally {
      setIsLoading(false);
    }
  }, [pageSize]);

  useEffect(() => {
    fetchOrders(page);
  }, [page, fetchOrders]);

  const handlePrevPage = () => {
    if (page > 0) setPage(page - 1);
  };

  const handleNextPage = () => {
    if (page < totalPages - 1) setPage(page + 1);
  };

  const handleRowClick = (orderId: string) => {
    navigate(`/orders/${orderId}`);
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'filled': return 'text-green-400';
      case 'partially_filled': return 'text-yellow-400';
      case 'cancelled': return 'text-red-400';
      case 'pending': return 'text-white/60';
      default: return 'text-white/40';
    }
  };

  const formatStatus = (status: string) => {
    return status.replace('_', ' ').replace(/\b\w/g, l => l.toUpperCase());
  };

  const py = compact ? 'py-2' : 'py-3';

  return (
    <div className="bg-zinc-900/50 border border-white/10 rounded-lg overflow-hidden">
      <div className="px-4 py-3 border-b border-white/10 flex items-center justify-between">
        <h2 className="text-sm font-medium">Order History</h2>
        <span className="text-xs text-white/40">{total} orders</span>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-white/10">
              <th className={`px-4 ${py} text-left text-xs font-medium text-white/60`}>Date</th>
              <th className={`px-4 ${py} text-left text-xs font-medium text-white/60`}>Pair</th>
              <th className={`px-4 ${py} text-left text-xs font-medium text-white/60`}>Type</th>
              <th className={`px-4 ${py} text-left text-xs font-medium text-white/60`}>Side</th>
              <th className={`px-4 ${py} text-right text-xs font-medium text-white/60`}>Price</th>
              <th className={`px-4 ${py} text-right text-xs font-medium text-white/60`}>Quantity</th>
              <th className={`px-4 ${py} text-right text-xs font-medium text-white/60`}>Filled</th>
              <th className={`px-4 ${py} text-left text-xs font-medium text-white/60`}>Status</th>
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              <tr>
                <td colSpan={8} className="px-4 py-8 text-center text-white/40 text-sm">
                  Loading...
                </td>
              </tr>
            ) : orders.length === 0 ? (
              <tr>
                <td colSpan={8} className="px-4 py-8 text-center text-white/40 text-sm">
                  No orders yet
                </td>
              </tr>
            ) : (
              orders.map((order) => {
                const date = new Date(order.created_at);
                const isBuy = order.side === 'bid';
                const filled = parseFloat(order.filled_quantity);
                const qty = parseFloat(order.quantity);
                const fillPercent = qty > 0 ? (filled / qty * 100).toFixed(0) : '0';

                return (
                  <tr
                    key={order.id}
                    onClick={() => handleRowClick(order.id)}
                    className={`border-b border-white/5 last:border-0 hover:bg-white/5 transition-colors cursor-pointer`}
                  >
                    <td className={`px-4 ${py} text-sm text-white/70 tabular-nums`}>
                      {date.toLocaleDateString()}{' '}
                      <span className="text-white/40">
                        {date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                      </span>
                    </td>
                    <td className={`px-4 ${py} text-sm font-medium`}>{order.symbol}</td>
                    <td className={`px-4 ${py} text-sm text-white/60 capitalize`}>{order.order_type}</td>
                    <td className={`px-4 ${py}`}>
                      <span className={`text-sm font-medium ${isBuy ? 'text-green-400' : 'text-red-400'}`}>
                        {isBuy ? 'BUY' : 'SELL'}
                      </span>
                    </td>
                    <td className={`px-4 ${py} text-sm text-right tabular-nums`}>
                      {order.price
                        ? parseFloat(order.price).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
                        : 'Market'}
                    </td>
                    <td className={`px-4 ${py} text-sm text-right tabular-nums text-white/70`}>
                      {qty.toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                    </td>
                    <td className={`px-4 ${py} text-sm text-right tabular-nums`}>
                      <span className="text-white/70">
                        {filled.toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                      </span>
                      <span className="text-white/40 ml-1">({fillPercent}%)</span>
                    </td>
                    <td className={`px-4 ${py} text-sm ${getStatusColor(order.status)}`}>
                      {formatStatus(order.status)}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {showPagination && totalPages > 1 && (
        <div className="px-4 py-3 border-t border-white/10 flex items-center justify-between">
          <div className="text-xs text-white/40">
            Page {page + 1} of {totalPages}
          </div>
          <div className="flex gap-2">
            <button
              onClick={handlePrevPage}
              disabled={page === 0}
              className="px-3 py-1.5 text-xs bg-white/5 border border-white/10 rounded hover:bg-white/10 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              Previous
            </button>
            <button
              onClick={handleNextPage}
              disabled={page >= totalPages - 1}
              className="px-3 py-1.5 text-xs bg-white/5 border border-white/10 rounded hover:bg-white/10 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
