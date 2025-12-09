# WebSocket Protocol

Thalex-inspired WebSocket protocol for real-time market data.

## Connection

Connect to: `ws://localhost:3000/ws`

## Client Messages (Client → Server)

### Subscribe to Channel

```json
{
  "action": "subscribe",
  "channel": "book.KCN/EUR.none.10.500ms"
}
```

### Unsubscribe from Channel

```json
{
  "action": "unsubscribe",
  "channel": "book.KCN/EUR.none.10.500ms"
}
```

## Server Messages (Server → Client)

### Channel Notification

```json
{
  "channel_name": "book.KCN/EUR.none.10.500ms",
  "notification": {
    "trades": [],
    "bid_changes": [[86498, 0.0, 0.0578]],
    "ask_changes": [[86498, 0.0578, 0.0]],
    "total_bid_amount": 2.5766,
    "total_ask_amount": 2.5236,
    "time": 1764625278.223605
  }
}
```

**Fields:**
- `channel_name`: Channel identifier (format: `book.{SYMBOL}.none.{DEPTH}.{INTERVAL}`)
- `notification.trades`: Array of recent trades (currently empty, will be populated)
- `notification.bid_changes`: Array of bid changes `[price, old_quantity, new_quantity]`
- `notification.ask_changes`: Array of ask changes `[price, old_quantity, new_quantity]`
- `notification.total_bid_amount`: Total bid quantity
- `notification.total_ask_amount`: Total ask quantity
- `notification.time`: Unix timestamp with microseconds

**Price Level Changes:**
- `[price, old_qty, new_qty]` - Price level update
- `[price, old_qty, 0]` - Price level removed
- `[price, 0, new_qty]` - New price level added

## Channel Format

`book.{SYMBOL}.none.{DEPTH}.{INTERVAL}`

- `SYMBOL`: Trading pair (e.g., `KCN/EUR`)
- `DEPTH`: Number of price levels (e.g., `10`)
- `INTERVAL`: Update interval (e.g., `500ms`)

Example: `book.KCN/EUR.none.10.500ms`

## Example Flow

1. Client connects to WebSocket
2. Client subscribes: `{"action": "subscribe", "channel": "book.KCN/EUR.none.10.500ms"}`
3. Server sends orderbook updates every 500ms with only changes (deltas)
4. Client receives incremental updates:
   ```json
   {
     "channel_name": "book.KCN/EUR.none.10.500ms",
     "notification": {
       "bid_changes": [[50000, 0.1, 0.15]],
       "ask_changes": [],
       ...
     }
   }
   ```

## Implementation Details

- **Incremental Updates**: Only sends changes (deltas), not full snapshots
- **Tick-based**: Updates sent at configured intervals (e.g., 500ms)
- **Channel Management**: Server tracks subscriptions per client
- **Automatic Cleanup**: Unsubscribes all channels when client disconnects


