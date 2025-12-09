import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { accountsAPI } from '../api/accounts';
import type { User, Balance } from '../api/accounts';

// Helper to check if error is a 401 Unauthorized
function isUnauthorizedError(e: unknown): boolean {
  return e instanceof Error && 'status' in e && (e as Error & { status?: number }).status === 401;
}

interface AuthState {
  user: User | null;
  accessToken: string | null;
  balances: Balance[];
  isLoading: boolean;
  error: string | null;
  lastRefreshAttempt: number | null;

  requestOtp: (email: string) => Promise<boolean>;
  verifyOtp: (email: string, code: string) => Promise<boolean>;
  signup: (email: string, code: string) => Promise<boolean>;
  logout: () => Promise<void>;
  refreshToken: () => Promise<boolean>;
  fetchBalances: () => Promise<void>;
  clearError: () => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      user: null,
      accessToken: null,
      balances: [],
      isLoading: false,
      error: null,
      lastRefreshAttempt: null,

      requestOtp: async (email: string) => {
        set({ isLoading: true, error: null });
        try {
          await accountsAPI.requestOtp(email);
          set({ isLoading: false });
          return true;
        } catch (e) {
          set({ isLoading: false, error: (e as Error).message });
          return false;
        }
      },

      verifyOtp: async (email: string, code: string) => {
        set({ isLoading: true, error: null });
        try {
          const { access_token, user } = await accountsAPI.verifyOtp(email, code);
          accountsAPI.setAccessToken(access_token);
          set({ user, accessToken: access_token, isLoading: false });
          // Fetch balances after login
          get().fetchBalances();
          return true;
        } catch (e) {
          set({ isLoading: false, error: (e as Error).message });
          return false;
        }
      },

      signup: async (email: string, code: string) => {
        set({ isLoading: true, error: null });
        try {
          const { access_token, user } = await accountsAPI.signup(email, code);
          accountsAPI.setAccessToken(access_token);
          set({ user, accessToken: access_token, isLoading: false });
          // Fetch balances after signup
          get().fetchBalances();
          return true;
        } catch (e) {
          set({ isLoading: false, error: (e as Error).message });
          return false;
        }
      },

      logout: async () => {
        try {
          await accountsAPI.logout();
        } catch {
          // Ignore logout errors
        }
        accountsAPI.setAccessToken(null);
        set({ user: null, accessToken: null, balances: [] });
      },

      refreshToken: async () => {
        const { lastRefreshAttempt } = get();
        const now = Date.now();

        // Prevent rapid refresh attempts (min 5 seconds between attempts)
        if (lastRefreshAttempt && now - lastRefreshAttempt < 5000) {
          console.log('[Auth] Skipping refresh - too soon since last attempt');
          return false;
        }

        set({ lastRefreshAttempt: now });

        try {
          const { access_token } = await accountsAPI.refresh();
          accountsAPI.setAccessToken(access_token);
          set({ accessToken: access_token, error: null });
          console.log('[Auth] Token refreshed successfully');
          return true;
        } catch (e) {
          // Refresh failed, clear auth state
          console.error('[Auth] Token refresh failed:', e);
          accountsAPI.setAccessToken(null);
          set({ user: null, accessToken: null, balances: [], error: 'Session expired. Please sign in again.' });
          return false;
        }
      },

      fetchBalances: async () => {
        const { accessToken } = get();
        if (!accessToken) return;

        try {
          const { balances } = await accountsAPI.getBalances();
          set({ balances });
        } catch (e) {
          // Only try refresh if it's a 401 error (token expired)
          if (isUnauthorizedError(e)) {
            console.log('[Auth] Got 401 on fetchBalances, attempting token refresh');
            const refreshed = await get().refreshToken();
            if (refreshed) {
              try {
                const { balances } = await accountsAPI.getBalances();
                set({ balances });
              } catch (retryError) {
                console.error('[Auth] fetchBalances retry failed:', retryError);
                // Don't silently swallow - set error state
                set({ error: 'Failed to fetch balances after token refresh' });
              }
            }
            // If refresh failed, refreshToken already set the error
          } else {
            // Non-auth error (network, server error, etc) - log but don't logout
            console.error('[Auth] fetchBalances failed (non-401):', e);
          }
        }
      },

      clearError: () => set({ error: null }),
    }),
    {
      name: 'auth-storage',
      partialize: (state) => ({
        user: state.user,
        accessToken: state.accessToken,
      }),
      onRehydrateStorage: () => (state, error) => {
        if (error) {
          console.error('[Auth] Rehydration error:', error);
          return;
        }

        // Restore token to API client after rehydration
        if (state?.accessToken) {
          accountsAPI.setAccessToken(state.accessToken);
          console.log('[Auth] Token restored from storage');

          // Proactively refresh token on page load to ensure it's valid
          // This prevents issues with stale tokens after long idle periods
          setTimeout(() => {
            console.log('[Auth] Proactive token refresh on rehydration');
            state.refreshToken().then((success) => {
              if (success) {
                state.fetchBalances();
              }
            });
          }, 100);
        }
      },
    }
  )
);
