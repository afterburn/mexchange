/**
 * High-Resolution Timer Web Worker
 *
 * Provides precise timing for trading bot strategies using performance.now()
 * instead of setInterval which is throttled by browsers.
 *
 * Benefits:
 * - Not throttled in background tabs (workers are not throttled like main thread)
 * - Uses performance.now() for microsecond precision
 * - Compensates for drift in timing
 */

type TimerId = number;

interface Timer {
  id: TimerId;
  interval: number;
  lastTick: number;
  callback: string; // Strategy name to execute
}

const timers: Map<TimerId, Timer> = new Map();
let nextId: TimerId = 1;
let isRunning = false;
let rafHandle: number | null = null;

// Message types
type WorkerInMessage =
  | { type: 'START_TIMER'; id?: TimerId; interval: number; callback: string }
  | { type: 'STOP_TIMER'; id: TimerId }
  | { type: 'STOP_ALL' };

interface TimerTickMessage {
  type: 'TICK';
  id: TimerId;
  callback: string;
  timestamp: number;
}

interface TimerStartedMessage {
  type: 'TIMER_STARTED';
  id: TimerId;
}

type WorkerOutMessage = TimerTickMessage | TimerStartedMessage;

/**
 * High-precision tick loop using performance.now()
 */
function tick(): void {
  if (!isRunning || timers.size === 0) {
    isRunning = false;
    return;
  }

  const now = performance.now();

  for (const [id, timer] of timers) {
    const elapsed = now - timer.lastTick;

    if (elapsed >= timer.interval) {
      // Calculate how many ticks should have occurred (handles drift)
      const tickCount = Math.floor(elapsed / timer.interval);

      // Update lastTick to compensate for drift
      timer.lastTick += tickCount * timer.interval;

      // Send tick message
      const message: TimerTickMessage = {
        type: 'TICK',
        id,
        callback: timer.callback,
        timestamp: now,
      };
      self.postMessage(message);
    }
  }

  // Schedule next check with minimal delay
  // Using setTimeout(0) gives ~4ms minimum, but in a worker it's more reliable
  // than on the main thread
  rafHandle = setTimeout(tick, 1) as unknown as number;
}

function startLoop(): void {
  if (isRunning) return;
  isRunning = true;
  tick();
}

function stopLoop(): void {
  isRunning = false;
  if (rafHandle !== null) {
    clearTimeout(rafHandle);
    rafHandle = null;
  }
}

// Message handler
self.onmessage = (event: MessageEvent<WorkerInMessage>) => {
  const message = event.data;

  switch (message.type) {
    case 'START_TIMER': {
      const id = message.id ?? nextId++;
      const timer: Timer = {
        id,
        interval: message.interval,
        lastTick: performance.now(),
        callback: message.callback,
      };
      timers.set(id, timer);

      const response: TimerStartedMessage = {
        type: 'TIMER_STARTED',
        id,
      };
      self.postMessage(response);

      startLoop();
      break;
    }

    case 'STOP_TIMER': {
      timers.delete(message.id);
      if (timers.size === 0) {
        stopLoop();
      }
      break;
    }

    case 'STOP_ALL': {
      timers.clear();
      stopLoop();
      break;
    }
  }
};

export type { WorkerInMessage, WorkerOutMessage, TimerTickMessage };
