import { useState, useRef, useEffect } from 'react';
import { ChevronDown, Star } from 'lucide-react';

type MarketType = 'spot' | 'futures';

interface Market {
  symbol: string;
  baseAsset: string;
  quoteAsset: string;
  price: number;
  change24h: number;
  favorite?: boolean;
}

const spotMarkets: Market[] = [
  { symbol: 'KCN/EUR', baseAsset: 'KCN', quoteAsset: 'EUR', price: 10.50, change24h: 2.34, favorite: true },
  { symbol: 'BTC/EUR', baseAsset: 'BTC', quoteAsset: 'EUR', price: 45230.00, change24h: -1.25 },
  { symbol: 'ETH/EUR', baseAsset: 'ETH', quoteAsset: 'EUR', price: 2340.00, change24h: 0.87 },
  { symbol: 'SOL/EUR', baseAsset: 'SOL', quoteAsset: 'EUR', price: 98.50, change24h: 5.23 },
];

const futuresMarkets: Market[] = [
  { symbol: 'KCN-PERP', baseAsset: 'KCN', quoteAsset: 'EUR', price: 10.52, change24h: 2.45, favorite: true },
  { symbol: 'BTC-PERP', baseAsset: 'BTC', quoteAsset: 'EUR', price: 45245.00, change24h: -1.18 },
  { symbol: 'ETH-PERP', baseAsset: 'ETH', quoteAsset: 'EUR', price: 2342.50, change24h: 0.92 },
  { symbol: 'SOL-PERP', baseAsset: 'SOL', quoteAsset: 'EUR', price: 98.65, change24h: 5.31 },
];

interface MarketSelectorProps {
  currentSymbol: string;
  currentPrice?: number;
  currentChange24h?: number;
  onSelectMarket?: (symbol: string) => void;
}

export default function MarketSelector({ currentSymbol, currentPrice, currentChange24h, onSelectMarket }: MarketSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<MarketType>('spot');
  const [searchQuery, setSearchQuery] = useState('');
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  const markets = activeTab === 'spot' ? spotMarkets : futuresMarkets;

  // Update current market with live data
  const marketsWithLiveData = markets.map(m => {
    if (m.symbol === currentSymbol && currentPrice !== undefined) {
      return { ...m, price: currentPrice, change24h: currentChange24h ?? m.change24h };
    }
    return m;
  });

  const filteredMarkets = marketsWithLiveData.filter(m =>
    m.symbol.toLowerCase().includes(searchQuery.toLowerCase()) ||
    m.baseAsset.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSelectMarket = (symbol: string) => {
    onSelectMarket?.(symbol);
    setIsOpen(false);
  };

  return (
    <div ref={containerRef} className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1.5 text-sm font-semibold hover:text-white/80 transition-colors"
      >
        {currentSymbol}
        <ChevronDown size={14} className={`transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      {isOpen && (
        <div className="absolute top-full left-0 mt-2 w-80 bg-zinc-900 border border-white/10 rounded-lg shadow-xl z-50 overflow-hidden">
          {/* Tabs */}
          <div className="flex border-b border-white/10">
            <button
              onClick={() => setActiveTab('spot')}
              className={`flex-1 py-2.5 text-xs font-medium transition-colors ${
                activeTab === 'spot'
                  ? 'text-white border-b-2 border-white'
                  : 'text-white/40 hover:text-white/60'
              }`}
            >
              Spot
            </button>
            <button
              onClick={() => setActiveTab('futures')}
              className={`flex-1 py-2.5 text-xs font-medium transition-colors ${
                activeTab === 'futures'
                  ? 'text-white border-b-2 border-white'
                  : 'text-white/40 hover:text-white/60'
              }`}
            >
              Futures
            </button>
          </div>

          {/* Search */}
          <div className="p-2 border-b border-white/10">
            <input
              type="text"
              placeholder="Search markets..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-white/5 border border-white/10 rounded px-3 py-1.5 text-xs text-white placeholder-white/30 focus:outline-none focus:border-white/20"
            />
          </div>

          {/* Market List */}
          <div className="max-h-64 overflow-y-auto">
            <table className="w-full">
              <thead>
                <tr className="text-[10px] text-white/40 border-b border-white/5">
                  <th className="text-left px-3 py-2 font-medium">Market</th>
                  <th className="text-right px-3 py-2 font-medium">Price</th>
                  <th className="text-right px-3 py-2 font-medium">24h %</th>
                </tr>
              </thead>
              <tbody>
                {filteredMarkets.map((market) => (
                  <tr
                    key={market.symbol}
                    onClick={() => handleSelectMarket(market.symbol)}
                    className={`hover:bg-white/5 cursor-pointer transition-colors ${
                      market.symbol === currentSymbol ? 'bg-white/5' : ''
                    }`}
                  >
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <Star
                          size={12}
                          className={market.favorite ? 'text-yellow-400 fill-yellow-400' : 'text-white/20'}
                        />
                        <span className="text-xs font-medium">{market.symbol}</span>
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right text-xs tabular-nums">
                      €{market.price.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                    </td>
                    <td className={`px-3 py-2 text-right text-xs tabular-nums ${
                      market.change24h >= 0 ? 'text-green-400' : 'text-red-400'
                    }`}>
                      {market.change24h >= 0 ? '+' : ''}{market.change24h.toFixed(2)}%
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

        </div>
      )}
    </div>
  );
}
