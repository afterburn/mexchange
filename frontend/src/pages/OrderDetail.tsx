import { useEffect, useState } from 'react';
import { Link, useParams, useNavigate } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { accountsAPI, type Order, type Trade } from '../api/accounts';
import logoSvg from '../assets/logo.svg';

export default function OrderDetail() {
  const { orderId } = useParams<{ orderId: string }>();
  const navigate = useNavigate();
  const { user, logout } = useAuthStore();
  const [order, setOrder] = useState<Order | null>(null);
  const [fills, setFills] = useState<Trade[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user) {
      navigate('/signin');
      return;
    }

    if (!orderId) return;

    const fetchData = async () => {
      setIsLoading(true);
      setError(null);
      try {
        const [orderData, fillsData] = await Promise.all([
          accountsAPI.getOrder(orderId),
          accountsAPI.getOrderFills(orderId),
        ]);
        setOrder(orderData);
        setFills(fillsData.fills);
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setIsLoading(false);
      }
    };

    fetchData();
  }, [user, orderId, navigate]);

  const handleLogout = async () => {
    await logout();
    navigate('/');
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'filled': return 'text-green-400 bg-green-400/10';
      case 'partially_filled': return 'text-yellow-400 bg-yellow-400/10';
      case 'cancelled': return 'text-red-400 bg-red-400/10';
      case 'pending': return 'text-white/60 bg-white/10';
      default: return 'text-white/40 bg-white/10';
    }
  };

  const formatStatus = (status: string) => {
    return status.replace('_', ' ').replace(/\b\w/g, l => l.toUpperCase());
  };

  if (!user) return null;

  return (
    <div className="min-h-screen bg-black text-white">
      {/* Top Navigation Bar */}
      <div className="flex items-center justify-between px-4 h-9 border-b border-white/10 shrink-0">
        <button
          onClick={() => navigate(-1)}
          className="flex items-center gap-2 text-xs text-white/60 hover:text-white transition-colors"
        >
          <span>&larr;</span>
          <span>Back</span>
        </button>
        <div />
      </div>

      {/* Auth Bar */}
      <div className="flex items-center justify-between px-4 h-8 border-b border-white/10 shrink-0">
        <Link to="/">
          <img src={logoSvg} alt="mExchange" className="h-5" />
        </Link>
        <div className="flex items-center gap-3">
          <Link to="/portfolio" className="text-xs text-white/60 hover:text-white transition-colors">
            Portfolio
          </Link>
          <span className="text-xs text-white/40">{user.email}</span>
          <button
            onClick={handleLogout}
            className="text-xs text-white/60 hover:text-white transition-colors"
          >
            Sign out
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="max-w-4xl mx-auto px-4 py-8">
        {isLoading ? (
          <div className="text-center text-white/40 py-12">Loading...</div>
        ) : error ? (
          <div className="text-center text-red-400 py-12">{error}</div>
        ) : order ? (
          <>
            {/* Order Summary */}
            <div className="bg-zinc-900/50 border border-white/10 rounded-lg overflow-hidden mb-6">
              <div className="px-4 py-3 border-b border-white/10 flex items-center justify-between">
                <h1 className="text-lg font-medium">Order Details</h1>
                <span className={`px-2 py-1 text-xs font-medium rounded ${getStatusColor(order.status)}`}>
                  {formatStatus(order.status)}
                </span>
              </div>

              <div className="p-4 grid grid-cols-2 md:grid-cols-4 gap-4">
                <div>
                  <div className="text-xs text-white/40 mb-1">Symbol</div>
                  <div className="text-sm font-medium">{order.symbol}</div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Side</div>
                  <div className={`text-sm font-medium ${order.side === 'bid' ? 'text-green-400' : 'text-red-400'}`}>
                    {order.side === 'bid' ? 'BUY' : 'SELL'}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Type</div>
                  <div className="text-sm capitalize">{order.order_type}</div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Price</div>
                  <div className="text-sm tabular-nums">
                    {order.price
                      ? parseFloat(order.price).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
                      : 'Market'}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Quantity</div>
                  <div className="text-sm tabular-nums">
                    {parseFloat(order.quantity).toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Filled</div>
                  <div className="text-sm tabular-nums">
                    {parseFloat(order.filled_quantity).toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                    <span className="text-white/40 ml-1">
                      ({(parseFloat(order.filled_quantity) / parseFloat(order.quantity) * 100).toFixed(0)}%)
                    </span>
                  </div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Remaining</div>
                  <div className="text-sm tabular-nums">
                    {(parseFloat(order.quantity) - parseFloat(order.filled_quantity)).toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-white/40 mb-1">Created</div>
                  <div className="text-sm tabular-nums">
                    {new Date(order.created_at).toLocaleString()}
                  </div>
                </div>
              </div>
            </div>

            {/* Fills Table */}
            <div className="bg-zinc-900/50 border border-white/10 rounded-lg overflow-hidden">
              <div className="px-4 py-3 border-b border-white/10 flex items-center justify-between">
                <h2 className="text-sm font-medium">Fills</h2>
                <span className="text-xs text-white/40">{fills.length} fills</span>
              </div>

              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-white/10">
                      <th className="px-4 py-3 text-left text-xs font-medium text-white/60">Time</th>
                      <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Price</th>
                      <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Quantity</th>
                      <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Total</th>
                      <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Fee</th>
                    </tr>
                  </thead>
                  <tbody>
                    {fills.length === 0 ? (
                      <tr>
                        <td colSpan={5} className="px-4 py-8 text-center text-white/40 text-sm">
                          No fills yet
                        </td>
                      </tr>
                    ) : (
                      fills.map((fill) => {
                        const date = new Date(fill.settled_at);
                        return (
                          <tr key={fill.id} className="border-b border-white/5 last:border-0">
                            <td className="px-4 py-3 text-sm text-white/70 tabular-nums">
                              {date.toLocaleDateString()}{' '}
                              <span className="text-white/40">
                                {date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                              </span>
                            </td>
                            <td className="px-4 py-3 text-sm text-right tabular-nums">
                              {parseFloat(fill.price).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                            </td>
                            <td className="px-4 py-3 text-sm text-right tabular-nums text-white/70">
                              {parseFloat(fill.quantity).toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                            </td>
                            <td className="px-4 py-3 text-sm text-right tabular-nums">
                              {parseFloat(fill.total).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                            </td>
                            <td className="px-4 py-3 text-sm text-right tabular-nums text-white/40">
                              {parseFloat(fill.fee).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 8 })}
                            </td>
                          </tr>
                        );
                      })
                    )}
                  </tbody>
                  {fills.length > 0 && (
                    <tfoot>
                      <tr className="border-t border-white/10 bg-white/5">
                        <td className="px-4 py-3 text-sm font-medium">Total</td>
                        <td className="px-4 py-3 text-sm text-right tabular-nums font-medium">
                          {fills.length > 0
                            ? (fills.reduce((sum, f) => sum + parseFloat(f.total), 0) / fills.reduce((sum, f) => sum + parseFloat(f.quantity), 0)).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
                            : '-'}
                          <span className="text-white/40 ml-1 font-normal">avg</span>
                        </td>
                        <td className="px-4 py-3 text-sm text-right tabular-nums font-medium">
                          {fills.reduce((sum, f) => sum + parseFloat(f.quantity), 0).toLocaleString(undefined, { minimumFractionDigits: 4, maximumFractionDigits: 8 })}
                        </td>
                        <td className="px-4 py-3 text-sm text-right tabular-nums font-medium">
                          {fills.reduce((sum, f) => sum + parseFloat(f.total), 0).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                        </td>
                        <td className="px-4 py-3 text-sm text-right tabular-nums font-medium text-white/60">
                          {fills.reduce((sum, f) => sum + parseFloat(f.fee), 0).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 8 })}
                        </td>
                      </tr>
                    </tfoot>
                  )}
                </table>
              </div>
            </div>
          </>
        ) : (
          <div className="text-center text-white/40 py-12">Order not found</div>
        )}
      </div>
    </div>
  );
}
