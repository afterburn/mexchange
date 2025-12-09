/**
 * Accounts API Client
 * Handles authentication and API calls to the accounts service
 */

const ACCOUNTS_URL = process.env.ACCOUNTS_URL || 'http://localhost:3001';
const GATEWAY_URL = process.env.GATEWAY_URL || 'http://localhost:3000';

// Use dev-login when available (ENVIRONMENT=development on accounts service)
const USE_DEV_LOGIN = process.env.USE_DEV_LOGIN !== 'false';

export interface AuthResponse {
  access_token: string;
  user: {
    id: string;
    email: string;
  };
}

export interface BalanceResponse {
  asset: string;
  available: string;
  locked: string;
}

export interface OrderResponse {
  id: string;
  symbol: string;
  side: string;
  order_type: string;
  price: string | null;
  quantity: string;
  filled_quantity: string;
  status: string;
  created_at: string;
}

export interface PlaceOrderResponse {
  order: OrderResponse;
  locked_asset: string;
  locked_amount: string;
}

export interface TradeResponse {
  id: string;
  symbol: string;
  side: string;
  price: string;
  quantity: string;
  total: string;
  fee: string;
  settled_at: string;
}

export class AccountsClient {
  private accessToken: string | null = null;
  private userId: string | null = null;
  private email: string | null = null;

  get isAuthenticated(): boolean {
    return this.accessToken !== null;
  }

  get currentUserId(): string | null {
    return this.userId;
  }

  get currentEmail(): string | null {
    return this.email;
  }

  /**
   * Request OTP for email - returns the OTP code from the database
   * In production this would be sent via email, but for testing we read directly from DB
   */
  async requestOtp(email: string): Promise<{ message: string }> {
    const response = await fetch(`${ACCOUNTS_URL}/auth/request-otp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Request OTP failed: ${error.error}`);
    }

    return response.json();
  }

  /**
   * Sign up with OTP verification
   */
  async signup(email: string, code: string): Promise<AuthResponse> {
    const response = await fetch(`${ACCOUNTS_URL}/auth/signup`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, code }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Signup failed: ${error.error}`);
    }

    const auth: AuthResponse = await response.json();
    this.accessToken = auth.access_token;
    this.userId = auth.user.id;
    this.email = auth.user.email;
    return auth;
  }

  /**
   * Sign in with OTP verification
   */
  async verifyOtp(email: string, code: string): Promise<AuthResponse> {
    const response = await fetch(`${ACCOUNTS_URL}/auth/verify-otp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, code }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Verify OTP failed: ${error.error}`);
    }

    const auth: AuthResponse = await response.json();
    this.accessToken = auth.access_token;
    this.userId = auth.user.id;
    this.email = auth.user.email;
    return auth;
  }

  /**
   * Dev-only: Login/signup without OTP (requires ENVIRONMENT=development on server)
   */
  async devLogin(email: string): Promise<AuthResponse> {
    const response = await fetch(`${ACCOUNTS_URL}/auth/dev-login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    });

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Dev login failed (${response.status}): ${text}`);
    }

    const auth: AuthResponse = await response.json();
    this.accessToken = auth.access_token;
    this.userId = auth.user.id;
    this.email = auth.user.email;
    return auth;
  }

  /**
   * Login - uses dev-login if available, otherwise OTP flow
   */
  async login(email: string): Promise<AuthResponse> {
    if (USE_DEV_LOGIN) {
      return this.devLogin(email);
    }
    // Fall back to OTP flow - caller needs to handle OTP code
    throw new Error('OTP login not supported in automated tests without dev-login');
  }

  private authHeaders(): Record<string, string> {
    if (!this.accessToken) {
      throw new Error('Not authenticated');
    }
    return {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${this.accessToken}`,
    };
  }

  /**
   * Get user balances
   */
  async getBalances(): Promise<BalanceResponse[]> {
    const response = await fetch(`${ACCOUNTS_URL}/api/balances`, {
      method: 'GET',
      headers: this.authHeaders(),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Get balances failed: ${error.error}`);
    }

    const result = await response.json();
    return result.balances;
  }

  /**
   * Deposit EUR
   */
  async deposit(amount: string): Promise<{ success: boolean; balance: BalanceResponse }> {
    const response = await fetch(`${ACCOUNTS_URL}/api/balances/deposit`, {
      method: 'POST',
      headers: this.authHeaders(),
      body: JSON.stringify({ asset: 'EUR', amount }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Deposit failed: ${error.error}`);
    }

    return response.json();
  }

  /**
   * Claim KCN from faucet
   */
  async claimFaucet(): Promise<{ success: boolean; amount: string; new_balance: string }> {
    const response = await fetch(`${ACCOUNTS_URL}/api/faucet/claim`, {
      method: 'POST',
      headers: this.authHeaders(),
      body: JSON.stringify({ asset: 'KCN' }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Faucet claim failed: ${error.error}`);
    }

    return response.json();
  }

  /**
   * Place an order (directly via accounts service - no matching engine)
   * Use placeOrderWithMatching() for orders that should be matched
   */
  async placeOrder(params: {
    symbol: string;
    side: 'bid' | 'ask';
    order_type: 'limit' | 'market';
    quantity: string;
    price?: string;
    max_slippage_price?: string;
  }): Promise<PlaceOrderResponse> {
    const response = await fetch(`${ACCOUNTS_URL}/api/orders`, {
      method: 'POST',
      headers: this.authHeaders(),
      body: JSON.stringify(params),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Place order failed: ${error.error}`);
    }

    return response.json();
  }

  /**
   * Place an order through the gateway (routes to matching engine for execution)
   * This is how the frontend places orders - they go through gateway which:
   * 1. Creates order in accounts service (locks funds)
   * 2. Forwards to matching engine for execution
   * 3. Settlement happens when fills are received
   */
  async placeOrderWithMatching(params: {
    symbol: string;
    side: 'bid' | 'ask';
    order_type: 'limit' | 'market';
    quantity: string;
    price?: string;
    max_slippage_price?: string;
  }): Promise<PlaceOrderResponse> {
    // Gateway endpoint for authenticated order placement
    const response = await fetch(`${GATEWAY_URL}/api/order`, {
      method: 'POST',
      headers: this.authHeaders(),
      body: JSON.stringify(params),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Place order failed: ${error.error}`);
    }

    // Gateway returns { order_id, status }
    const gatewayResponse: { order_id: string; status: string } = await response.json();

    // Fetch full order details from accounts service
    const order = await this.getOrder(gatewayResponse.order_id);

    // Calculate locked amount (approximation - actual is in the ledger)
    let lockedAsset: string;
    let lockedAmount: string;

    if (params.side === 'ask') {
      // Selling: lock base asset (e.g., KCN)
      lockedAsset = params.symbol.split('/')[0];
      lockedAmount = params.quantity;
    } else {
      // Buying: lock quote asset (e.g., EUR)
      lockedAsset = params.symbol.split('/')[1];
      const price = params.price || params.max_slippage_price || '1000';
      lockedAmount = (parseFloat(params.quantity) * parseFloat(price)).toString();
    }

    return {
      order,
      locked_asset: lockedAsset,
      locked_amount: lockedAmount,
    };
  }

  /**
   * Get user orders
   */
  async getOrders(): Promise<OrderResponse[]> {
    const response = await fetch(`${ACCOUNTS_URL}/api/orders`, {
      method: 'GET',
      headers: this.authHeaders(),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Get orders failed: ${error.error}`);
    }

    const result = await response.json();
    return result.orders;
  }

  /**
   * Get specific order
   */
  async getOrder(orderId: string): Promise<OrderResponse> {
    const response = await fetch(`${ACCOUNTS_URL}/api/orders/${orderId}`, {
      method: 'GET',
      headers: this.authHeaders(),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Get order failed: ${error.error}`);
    }

    return response.json();
  }

  /**
   * Cancel an order
   */
  async cancelOrder(orderId: string): Promise<{ order: OrderResponse }> {
    const response = await fetch(`${ACCOUNTS_URL}/api/orders/${orderId}`, {
      method: 'DELETE',
      headers: this.authHeaders(),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Cancel order failed: ${error.error}`);
    }

    return response.json();
  }

  /**
   * Get user trades
   */
  async getTrades(): Promise<TradeResponse[]> {
    const response = await fetch(`${ACCOUNTS_URL}/api/orders/trades`, {
      method: 'GET',
      headers: this.authHeaders(),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(`Get trades failed: ${error.error}`);
    }

    const result = await response.json();
    return result.trades;
  }

  /**
   * Clear auth state
   */
  logout(): void {
    this.accessToken = null;
    this.userId = null;
    this.email = null;
  }
}
