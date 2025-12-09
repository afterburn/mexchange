const WebSocket = require('ws');

const ws = new WebSocket('ws://localhost:3000/ws');

ws.on('open', function open() {
  console.log('CONNECTED');

  const msg = {
    type: 'orders',
    orders: [
      { side: 'bid', order_type: 'limit', price: 41.50, quantity: 10.0 }
    ]
  };
  console.log('SENDING:', JSON.stringify(msg));
  ws.send(JSON.stringify(msg));
  console.log('SENT');

  setTimeout(() => {
    ws.close();
    process.exit(0);
  }, 3000);
});

ws.on('message', function message(data) {
  console.log('RECEIVED:', data.toString().slice(0, 300));
});

ws.on('error', function error(err) {
  console.error('ERROR:', err.message);
  process.exit(1);
});

ws.on('close', function close() {
  console.log('DISCONNECTED');
});
