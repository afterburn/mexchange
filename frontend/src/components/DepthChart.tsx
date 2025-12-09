import { useMemo, memo } from 'react';
import type { OrderBookLevel } from '../types';

function formatVolume(vol: number): string {
  if (vol >= 1_000_000) return `${(vol / 1_000_000).toFixed(1)}M`;
  if (vol >= 1_000) return `${(vol / 1_000).toFixed(1)}K`;
  if (vol >= 1) return vol.toFixed(0);
  return vol.toFixed(2);
}

interface DepthChartProps {
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}

const DepthChart = memo(function DepthChart({ bids, asks }: DepthChartProps) {
  const chartData = useMemo(() => {
    if (bids.length === 0 && asks.length === 0) return null;

    // Sort bids high to low (best bid first), asks low to high (best ask first)
    const sortedBids = [...bids].sort((a, b) => b.price - a.price);
    const sortedAsks = [...asks].sort((a, b) => a.price - b.price);

    // Bids: cumulative from best bid (highest) going down
    // At best bid, cumulative = just that level. At lower prices, cumulative increases.
    const bidPoints = sortedBids.reduce<Array<{ price: number; cumulative: number }>>((acc, bid) => {
      const prevCumulative = acc.length > 0 ? acc[acc.length - 1].cumulative : 0;
      acc.push({ price: bid.price, cumulative: prevCumulative + bid.quantity });
      return acc;
    }, []);

    // Asks: cumulative from best ask (lowest) going up
    // At best ask, cumulative = just that level. At higher prices, cumulative increases.
    const askPoints = sortedAsks.reduce<Array<{ price: number; cumulative: number }>>((acc, ask) => {
      const prevCumulative = acc.length > 0 ? acc[acc.length - 1].cumulative : 0;
      acc.push({ price: ask.price, cumulative: prevCumulative + ask.quantity });
      return acc;
    }, []);

    // Calculate mid price
    const midPrice = sortedBids[0]?.price && sortedAsks[0]?.price
      ? (sortedBids[0].price + sortedAsks[0].price) / 2
      : null;

    // Get price range centered around mid price
    const allPrices = [...bidPoints.map(p => p.price), ...askPoints.map(p => p.price)];
    const minPrice = Math.min(...allPrices);
    const maxPrice = Math.max(...allPrices);

    // Calculate symmetric range around mid price
    let chartMinPrice: number;
    let chartMaxPrice: number;

    if (midPrice !== null) {
      const maxDistance = Math.max(midPrice - minPrice, maxPrice - midPrice);
      const padding = maxDistance * 0.1;
      chartMinPrice = midPrice - maxDistance - padding;
      chartMaxPrice = midPrice + maxDistance + padding;
    } else {
      const priceRange = maxPrice - minPrice || 1;
      const pricePadding = priceRange * 0.05;
      chartMinPrice = minPrice - pricePadding;
      chartMaxPrice = maxPrice + pricePadding;
    }

    const chartPriceRange = chartMaxPrice - chartMinPrice;

    // Get max cumulative volume
    const maxVolume = Math.max(
      bidPoints[bidPoints.length - 1]?.cumulative || 0,
      askPoints[askPoints.length - 1]?.cumulative || 0
    );

    return {
      bidPoints,
      askPoints,
      chartMinPrice,
      chartMaxPrice,
      chartPriceRange,
      maxVolume,
      midPrice: midPrice ?? (minPrice + maxPrice) / 2,
    };
  }, [bids, asks]);

  if (!chartData) {
    return (
      <div className="flex items-center justify-center h-full bg-black text-white/40">
        <div>Waiting for order book data...</div>
      </div>
    );
  }

  const { bidPoints, askPoints, chartMinPrice, chartPriceRange, maxVolume, midPrice } = chartData;

  // SVG dimensions
  const width = 100;
  const height = 100;
  const padding = { top: 5, right: 5, bottom: 5, left: 5 };
  const chartWidth = width - padding.left - padding.right;
  const chartHeight = height - padding.top - padding.bottom;

  // Scale functions
  const scaleX = (price: number) => {
    return padding.left + ((price - chartMinPrice) / chartPriceRange) * chartWidth;
  };

  const scaleY = (volume: number) => {
    return padding.top + chartHeight - (volume / maxVolume) * chartHeight;
  };

  // Build stepped path for bids (green)
  // bidPoints: [0] = best bid (highest price, smallest cumulative), [n] = lowest price (largest cumulative)
  // Path goes from best bid (right side, near mid) to lowest bid (left side)
  const buildBidPath = () => {
    if (bidPoints.length === 0) return '';

    const points: string[] = [];

    // Start at bottom at best bid price (right side of bids)
    points.push(`M ${scaleX(bidPoints[0].price)} ${scaleY(0)}`);

    for (let i = 0; i < bidPoints.length; i++) {
      const point = bidPoints[i];
      // Vertical line up to cumulative
      points.push(`L ${scaleX(point.price)} ${scaleY(point.cumulative)}`);
      // Horizontal line to next price (going left to lower prices)
      if (i < bidPoints.length - 1) {
        points.push(`L ${scaleX(bidPoints[i + 1].price)} ${scaleY(point.cumulative)}`);
      }
    }

    // Close the path back to baseline at lowest bid
    const lastBid = bidPoints[bidPoints.length - 1];
    points.push(`L ${scaleX(lastBid.price)} ${scaleY(0)}`);
    points.push('Z');

    return points.join(' ');
  };

  // Build stepped path for asks (red)
  // askPoints: [0] = best ask (lowest price, smallest cumulative), [n] = highest price (largest cumulative)
  // Path goes from best ask (left side, near mid) to highest ask (right side)
  const buildAskPath = () => {
    if (askPoints.length === 0) return '';

    const points: string[] = [];

    // Start at bottom at best ask price (left side of asks, near mid)
    points.push(`M ${scaleX(askPoints[0].price)} ${scaleY(0)}`);

    for (let i = 0; i < askPoints.length; i++) {
      const point = askPoints[i];
      // Vertical line up to cumulative
      points.push(`L ${scaleX(point.price)} ${scaleY(point.cumulative)}`);
      // Horizontal line to next price (going right to higher prices)
      if (i < askPoints.length - 1) {
        points.push(`L ${scaleX(askPoints[i + 1].price)} ${scaleY(point.cumulative)}`);
      }
    }

    // Close the path back to baseline at highest ask
    const lastAsk = askPoints[askPoints.length - 1];
    points.push(`L ${scaleX(lastAsk.price)} ${scaleY(0)}`);
    points.push('Z');

    return points.join(' ');
  };

  // Generate price axis labels
  const priceLabels = [];
  const numPriceLabels = 7;
  for (let i = 0; i <= numPriceLabels; i++) {
    const price = chartMinPrice + (chartPriceRange * i) / numPriceLabels;
    priceLabels.push({ price, x: scaleX(price) });
  }

  // Generate volume axis labels
  const volumeLabels = [];
  const numVolumeLabels = 5;
  for (let i = 0; i <= numVolumeLabels; i++) {
    const volume = (maxVolume * i) / numVolumeLabels;
    volumeLabels.push({ volume, y: scaleY(volume) });
  }

  return (
    <div className="h-full bg-black flex flex-col p-4">
      <div className="flex-1 relative">
        <svg
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="none"
          className="w-full h-full"
        >
          {/* Grid lines */}
          <g className="text-white/10">
            {priceLabels.map((label, i) => (
              <line
                key={`v-${i}`}
                x1={label.x}
                y1={padding.top}
                x2={label.x}
                y2={height - padding.bottom}
                stroke="currentColor"
                strokeWidth="0.2"
              />
            ))}
            {volumeLabels.map((label, i) => (
              <line
                key={`h-${i}`}
                x1={padding.left}
                y1={label.y}
                x2={width - padding.right}
                y2={label.y}
                stroke="currentColor"
                strokeWidth="0.2"
              />
            ))}
          </g>

          {/* Mid price line */}
          <line
            x1={scaleX(midPrice)}
            y1={padding.top}
            x2={scaleX(midPrice)}
            y2={height - padding.bottom}
            stroke="rgba(255,255,255,0.3)"
            strokeWidth="0.3"
            strokeDasharray="1,1"
          />

          {/* Bid area (green) */}
          <path
            d={buildBidPath()}
            fill="rgba(34, 197, 94, 0.3)"
            stroke="#22c55e"
            strokeWidth="1.5"
            vectorEffect="non-scaling-stroke"
          />

          {/* Ask area (red) */}
          <path
            d={buildAskPath()}
            fill="rgba(239, 68, 68, 0.3)"
            stroke="#ef4444"
            strokeWidth="1.5"
            vectorEffect="non-scaling-stroke"
          />
        </svg>

        {/* Price labels (bottom) */}
        <div className="absolute bottom-0 left-0 right-0 flex justify-between text-[10px] text-white/40 px-1">
          {priceLabels.filter((_, i) => i % 2 === 0).map((label, i) => (
            <span key={i}>€{label.price.toFixed(2)}</span>
          ))}
        </div>

        {/* Volume labels (left) */}
        <div className="absolute top-0 left-1 bottom-4 flex flex-col justify-between text-[10px] text-white/40">
          {volumeLabels.slice().reverse().map((label, i) => (
            <span key={i}>{formatVolume(label.volume)}</span>
          ))}
        </div>
      </div>
    </div>
  );
});

export default DepthChart;
