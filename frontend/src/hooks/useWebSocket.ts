import { useEffect, useRef, useState, useCallback } from 'react';

interface ChannelNotification {
  channel_name: string;
  notification: {
    trades: Array<{
      price: number;
      quantity: number;
      side: string;
      timestamp: number;
    }>;
    bid_changes: Array<[number, number, number]>;
    ask_changes: Array<[number, number, number]>;
    total_bid_amount: number;
    total_ask_amount: number;
    time: number;
  };
}

// Direct market events from the gateway
export interface OrderFilledEvent {
  type: 'order_filled';
  order_id: string;
}

export interface OrderCancelledEvent {
  type: 'order_cancelled';
  order_id: string;
  filled_quantity: string;
}

export type OrderEvent = OrderFilledEvent | OrderCancelledEvent;

type MessageHandler = (data: ChannelNotification) => void;
type OrderEventHandler = (event: OrderEvent) => void;

export function useWebSocket(url: string, onMessage: MessageHandler, onOrderEvent?: OrderEventHandler) {
  const [isConnected, setIsConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const onMessageRef = useRef(onMessage);
  const onOrderEventRef = useRef(onOrderEvent);
  const subscribedChannelsRef = useRef<Set<string>>(new Set());
  const pendingSubscriptionsRef = useRef<Set<string>>(new Set());
  const mountedRef = useRef(true);

  // Keep message handler refs updated
  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    onOrderEventRef.current = onOrderEvent;
  }, [onOrderEvent]);

  useEffect(() => {
    mountedRef.current = true;

    const connect = () => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        return;
      }

      try {
        console.log('[WebSocket] Creating new WebSocket to', url);
        const ws = new WebSocket(url);
        wsRef.current = ws;
        console.log('[WebSocket] wsRef.current set to', ws);

        ws.onopen = () => {
          console.log('[WebSocket] onopen fired, mountedRef:', mountedRef.current);
          if (!mountedRef.current) return;

          console.log('[WebSocket] Connection established');
          setIsConnected(true);

          // Process pending subscriptions
          pendingSubscriptionsRef.current.forEach(channel => {
            ws.send(JSON.stringify({ action: 'subscribe', channel }));
            subscribedChannelsRef.current.add(channel);
          });
          pendingSubscriptionsRef.current.clear();
        };

        ws.onmessage = (event) => {
          if (!mountedRef.current) return;
          try {
            const data = JSON.parse(event.data);

            // Check if this is an order lifecycle event (order_filled, order_cancelled)
            if (data.type === 'order_filled' || data.type === 'order_cancelled') {
              if (onOrderEventRef.current) {
                onOrderEventRef.current(data as OrderEvent);
              }
              return;
            }

            // Otherwise treat as channel notification
            if (data.channel_name && data.notification) {
              onMessageRef.current(data as ChannelNotification);
            }
          } catch {
            // Silently ignore parse errors in production
          }
        };

        ws.onerror = () => {
          // Error handling without logging
        };

        ws.onclose = () => {
          if (!mountedRef.current) return;

          setIsConnected(false);
          subscribedChannelsRef.current.clear();

          if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current);
          }

          reconnectTimeoutRef.current = window.setTimeout(() => {
            if (mountedRef.current) {
              connect();
            }
          }, 3000);
        };
      } catch {
        if (mountedRef.current) {
          setIsConnected(false);
        }
      }
    };

    connect();

    return () => {
      mountedRef.current = false;
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [url]);

  const subscribe = useCallback((channel: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      if (!subscribedChannelsRef.current.has(channel)) {
        wsRef.current.send(JSON.stringify({ action: 'subscribe', channel }));
        subscribedChannelsRef.current.add(channel);
        pendingSubscriptionsRef.current.delete(channel);
      }
    } else {
      pendingSubscriptionsRef.current.add(channel);
    }
  }, []);

  const unsubscribe = useCallback((channel: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ action: 'unsubscribe', channel }));
      subscribedChannelsRef.current.delete(channel);
    }
  }, []);

  // Not using useCallback - we want this to always use current wsRef
  const send = (message: unknown) => {
    console.log('[WebSocket] send called, wsRef.current:', wsRef.current, 'readyState:', wsRef.current?.readyState);
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      const json = JSON.stringify(message);
      console.log('[WebSocket] Actually sending to socket:', json.slice(0, 200));
      wsRef.current.send(json);
      return true;
    }
    console.log('[WebSocket] Cannot send - not connected');
    return false;
  };

  return { isConnected, subscribe, unsubscribe, send };
}
