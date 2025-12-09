import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuthStore } from '../stores/authStore';
import { accountsAPI, type FaucetStatus } from '../api/accounts';
import EmbedTopBar from '../components/EmbedTopBar';
import HeaderBar from '../components/HeaderBar';

type Tab = 'deposit' | 'withdraw';

export default function Portfolio() {
  const navigate = useNavigate();
  const { user, balances, fetchBalances } = useAuthStore();
  const [faucetStatus, setFaucetStatus] = useState<FaucetStatus | null>(null);
  const [faucetLoading, setFaucetLoading] = useState(false);
  const [faucetMessage, setFaucetMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  // Wallet state
  const [tab, setTab] = useState<Tab>('deposit');
  const [amount, setAmount] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    if (!user) {
      navigate('/signin');
      return;
    }
    fetchBalances();
    loadFaucetStatus();
  }, [user, navigate, fetchBalances]);

  const loadFaucetStatus = async () => {
    try {
      const status = await accountsAPI.getFaucetStatus();
      setFaucetStatus(status);
    } catch (e) {
      console.error('Failed to load faucet status:', e);
    }
  };

  const handleClaimFaucet = async () => {
    setFaucetLoading(true);
    setFaucetMessage(null);
    try {
      const result = await accountsAPI.claimFaucet();
      setFaucetMessage({ type: 'success', text: `Claimed ${result.amount} KCN!` });
      fetchBalances();
      loadFaucetStatus();
    } catch (e) {
      setFaucetMessage({ type: 'error', text: (e as Error).message });
    } finally {
      setFaucetLoading(false);
    }
  };

  const formatTimeRemaining = (nextClaimAt: string) => {
    const diff = new Date(nextClaimAt).getTime() - Date.now();
    if (diff <= 0) return 'Available now';
    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    return `${hours}h ${minutes}m`;
  };

  const eurBalance = balances.find(b => b.asset === 'EUR');
  const availableEur = eurBalance ? parseFloat(eurBalance.available) : 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccess(null);

    const amountNum = parseFloat(amount);
    if (isNaN(amountNum) || amountNum <= 0) {
      setError('Please enter a valid amount');
      return;
    }

    if (tab === 'withdraw' && amountNum > availableEur) {
      setError('Insufficient balance');
      return;
    }

    setIsLoading(true);
    try {
      if (tab === 'deposit') {
        await accountsAPI.deposit('EUR', amount);
        setSuccess(`Successfully deposited €${amountNum.toLocaleString('en-US', { minimumFractionDigits: 2 })}`);
      } else {
        await accountsAPI.withdraw('EUR', amount);
        setSuccess(`Successfully withdrew €${amountNum.toLocaleString('en-US', { minimumFractionDigits: 2 })}`);
      }
      setAmount('');
      fetchBalances();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setIsLoading(false);
    }
  };

  if (!user) return null;

  return (
    <div className="min-h-screen bg-black text-white">
      <EmbedTopBar />

      <HeaderBar onLogout={() => navigate('/')} />

      {/* Content */}
      <div className="max-w-4xl mx-auto px-4 py-8">
        <h1 className="text-2xl font-medium mb-6">Portfolio</h1>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Left Column: Balances */}
          <div className="space-y-6">
            {/* Balances Table */}
            <div className="bg-zinc-900/50 border border-white/10 rounded-lg overflow-hidden">
              <div className="px-4 py-3 border-b border-white/10">
                <h2 className="text-sm font-medium">Balances</h2>
              </div>
              <table className="w-full">
                <thead>
                  <tr className="border-b border-white/10">
                    <th className="px-4 py-3 text-left text-xs font-medium text-white/60">Asset</th>
                    <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Available</th>
                    <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Locked</th>
                    <th className="px-4 py-3 text-right text-xs font-medium text-white/60">Total</th>
                  </tr>
                </thead>
                <tbody>
                  {balances.length === 0 ? (
                    <tr>
                      <td colSpan={4} className="px-4 py-8 text-center text-white/40 text-sm">
                        No balances yet
                      </td>
                    </tr>
                  ) : (
                    balances.map((balance) => {
                      const available = parseFloat(balance.available);
                      const locked = parseFloat(balance.locked);
                      const total = available + locked;
                      const isFiat = ['EUR', 'USD', 'GBP'].includes(balance.asset);
                      const decimals = isFiat ? 2 : 8;
                      return (
                        <tr key={balance.asset} className="border-b border-white/5 last:border-0">
                          <td className="px-4 py-3 text-sm font-medium">{balance.asset}</td>
                          <td className="px-4 py-3 text-sm text-right tabular-nums">
                            {available.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: decimals })}
                          </td>
                          <td className="px-4 py-3 text-sm text-right tabular-nums text-white/60">
                            {locked.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: decimals })}
                          </td>
                          <td className="px-4 py-3 text-sm text-right tabular-nums">
                            {total.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: decimals })}
                          </td>
                        </tr>
                      );
                    })
                  )}
                </tbody>
              </table>
            </div>

            {/* KCN Faucet Card */}
            <div className="bg-gradient-to-r from-purple-900/30 to-blue-900/30 border border-purple-500/20 rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-medium text-purple-300">KCN Faucet</h3>
                  <p className="text-xs text-white/60 mt-1">
                    Claim free KCN tokens for testing ({faucetStatus?.amount_per_claim || '100'} KCN every {faucetStatus?.cooldown_hours || 24}h)
                  </p>
                  {faucetStatus && !faucetStatus.available && faucetStatus.next_claim_at && (
                    <p className="text-xs text-white/40 mt-1">
                      Next claim: {formatTimeRemaining(faucetStatus.next_claim_at)}
                    </p>
                  )}
                </div>
                <button
                  onClick={handleClaimFaucet}
                  disabled={faucetLoading || (faucetStatus !== null && !faucetStatus.available)}
                  className="px-4 py-2 text-sm bg-purple-500 text-white rounded hover:bg-purple-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {faucetLoading ? 'Claiming...' : 'Claim KCN'}
                </button>
              </div>
              {faucetMessage && (
                <div className={`mt-3 text-xs ${faucetMessage.type === 'success' ? 'text-green-400' : 'text-red-400'}`}>
                  {faucetMessage.text}
                </div>
              )}
            </div>
          </div>

          {/* Right Column: Deposit/Withdraw */}
          <div className="bg-zinc-900/50 border border-white/10 rounded-lg overflow-hidden">
            <div className="px-4 py-3 border-b border-white/10">
              <h2 className="text-sm font-medium">Deposit / Withdraw</h2>
            </div>

            {/* Tabs */}
            <div className="flex border-b border-white/10">
              <button
                onClick={() => { setTab('deposit'); setError(null); setSuccess(null); }}
                className={`flex-1 py-3 text-sm font-medium transition-colors ${
                  tab === 'deposit'
                    ? 'text-green-400 border-b-2 border-green-400'
                    : 'text-white/40 hover:text-white/60'
                }`}
              >
                Deposit
              </button>
              <button
                onClick={() => { setTab('withdraw'); setError(null); setSuccess(null); }}
                className={`flex-1 py-3 text-sm font-medium transition-colors ${
                  tab === 'withdraw'
                    ? 'text-red-400 border-b-2 border-red-400'
                    : 'text-white/40 hover:text-white/60'
                }`}
              >
                Withdraw
              </button>
            </div>

            <div className="p-4">
              {/* Balance Display */}
              <div className="bg-white/5 border border-white/10 rounded-lg p-3 mb-4">
                <div className="text-xs text-white/60 mb-1">Available EUR Balance</div>
                <div className="text-xl font-medium">
                  €{availableEur.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                </div>
              </div>

              {/* Form */}
              <form onSubmit={handleSubmit} className="space-y-4">
                <div>
                  <label className="block text-xs text-white/60 mb-2">Amount (EUR)</label>
                  <div className="relative">
                    <span className="absolute left-3 top-1/2 -translate-y-1/2 text-white/40">€</span>
                    <input
                      type="number"
                      step="0.01"
                      min="0"
                      value={amount}
                      onChange={(e) => setAmount(e.target.value)}
                      placeholder="0.00"
                      className="w-full bg-white/5 border border-white/10 rounded-lg pl-8 pr-4 py-3 text-white placeholder-white/30 focus:outline-none focus:border-white/30 transition-colors"
                    />
                  </div>
                  {tab === 'withdraw' && (
                    <div className="flex gap-2 mt-2">
                      {[25, 50, 75, 100].map((pct) => (
                        <button
                          key={pct}
                          type="button"
                          onClick={() => setAmount((availableEur * pct / 100).toFixed(2))}
                          className="flex-1 py-1.5 text-xs bg-white/5 border border-white/10 rounded hover:bg-white/10 transition-colors"
                        >
                          {pct}%
                        </button>
                      ))}
                    </div>
                  )}
                </div>

                {/* Demo Notice */}
                <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-3 text-xs text-blue-400">
                  {tab === 'deposit'
                    ? 'Demo mode: Funds are credited instantly without actual payment processing.'
                    : 'Demo mode: Funds are debited instantly without actual bank transfer.'}
                </div>

                {error && (
                  <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-3 text-xs text-red-400">
                    {error}
                  </div>
                )}

                {success && (
                  <div className="bg-green-500/10 border border-green-500/20 rounded-lg p-3 text-xs text-green-400">
                    {success}
                  </div>
                )}

                <button
                  type="submit"
                  disabled={isLoading || !amount}
                  className={`w-full py-3 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                    tab === 'deposit'
                      ? 'bg-green-500 text-white hover:bg-green-600'
                      : 'bg-red-500 text-white hover:bg-red-600'
                  }`}
                >
                  {isLoading ? 'Processing...' : tab === 'deposit' ? 'Deposit' : 'Withdraw'}
                </button>
              </form>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
