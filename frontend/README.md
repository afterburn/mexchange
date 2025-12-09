# mExchange Frontend

React + TypeScript + Tailwind CSS 4 frontend for the mExchange matching engine.

## Features

- **Order Book**: Real-time display of buy and sell orders with depth visualization
- **Price Chart**: Interactive price chart with multiple timeframes
- **Trade Form**: Place limit and market orders with percentage-based quantity selection
- **Recent Trades**: Live feed of recent trades
- **Open Orders**: View and manage your open orders
- **Market Statistics**: 24h price change, volume, high/low

## Getting Started

```bash
npm install
npm run dev
```

The app will be available at `http://localhost:5173`

## Project Structure

```
src/
  components/     # React components
  hooks/          # Custom React hooks
  types.ts        # TypeScript type definitions
  App.tsx         # Main application component
  main.tsx        # Application entry point
```

## Components

- `OrderBook`: Displays bids and asks with depth visualization
- `PriceChart`: SVG-based price chart
- `TradeForm`: Order placement form with buy/sell tabs
- `RecentTrades`: Recent trades feed
- `OpenOrders`: List of open orders with cancel functionality
- `MarketStats`: Market statistics header

## API Integration

The frontend currently uses mock data via the `useOrderBook` hook. To connect to a real backend:

1. Update `src/hooks/useOrderBook.ts` to fetch from your API
2. Implement WebSocket connections for real-time updates
3. Update the `placeOrder` and `cancelOrder` functions to call your backend

## Styling

Uses Tailwind CSS 4 with custom theme variables matching Binance's dark theme.
