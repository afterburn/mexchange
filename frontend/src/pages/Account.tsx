import { useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { User, Shield, Bell, Key, CreditCard, HelpCircle } from 'lucide-react';
import { useAuthStore } from '../stores/authStore';
import EmbedTopBar from '../components/EmbedTopBar';
import HeaderBar from '../components/HeaderBar';

type SettingsSection = 'general' | 'security' | 'notifications' | 'api-keys' | 'billing' | 'help';

const sidebarItems: { id: SettingsSection; label: string; icon: typeof User }[] = [
  { id: 'general', label: 'General', icon: User },
  { id: 'security', label: 'Security', icon: Shield },
  { id: 'notifications', label: 'Notifications', icon: Bell },
  { id: 'api-keys', label: 'API Keys', icon: Key },
  { id: 'billing', label: 'Billing', icon: CreditCard },
  { id: 'help', label: 'Help', icon: HelpCircle },
];

const validSections = new Set<string>(sidebarItems.map(item => item.id));

export default function Account() {
  const navigate = useNavigate();
  const { section } = useParams<{ section?: string }>();
  const { user } = useAuthStore();

  const activeSection: SettingsSection = section && validSections.has(section)
    ? section as SettingsSection
    : 'general';

  useEffect(() => {
    if (!user) {
      navigate('/signin');
    }
  }, [user, navigate]);

  if (!user) return null;

  return (
    <div className="min-h-screen bg-black text-white">
      <EmbedTopBar />
      <HeaderBar onLogout={() => navigate('/')} />

      <div className="max-w-5xl mx-auto px-4 py-8">
        <h1 className="text-2xl font-medium mb-6">Account Settings</h1>

        <div className="flex gap-8">
          {/* Sidebar */}
          <div className="w-48 shrink-0">
            <nav className="space-y-1">
              {sidebarItems.map((item) => {
                const Icon = item.icon;
                const isActive = activeSection === item.id;
                return (
                  <button
                    key={item.id}
                    onClick={() => navigate(`/account/${item.id}`)}
                    className={`w-full flex items-center gap-3 px-3 py-2 text-sm rounded-lg transition-colors ${
                      isActive
                        ? 'bg-white/10 text-white'
                        : 'text-white/60 hover:text-white hover:bg-white/5'
                    }`}
                  >
                    <Icon size={16} />
                    {item.label}
                  </button>
                );
              })}
            </nav>
          </div>

          {/* Content */}
          <div className="flex-1 min-w-0">
            {/* Demo Notice */}
            <div className="mb-6 bg-blue-500/10 border border-blue-500/20 rounded-lg p-3 text-sm text-blue-400">
              This is a demo account. Some settings are limited in demo mode.
            </div>

            {activeSection === 'general' && (
              <div className="space-y-6">
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">Profile</h2>
                  <div className="space-y-4">
                    <div>
                      <label className="block text-xs text-white/60 mb-1">Email</label>
                      <p className="text-sm">{user.email}</p>
                    </div>
                    <div>
                      <label className="block text-xs text-white/60 mb-1">User ID</label>
                      <p className="text-xs text-white/40 font-mono">{user.id}</p>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {activeSection === 'security' && (
              <div className="space-y-6">
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">Password</h2>
                  <p className="text-sm text-white/60 mb-4">Change your password to keep your account secure.</p>
                  <button className="px-4 py-2 text-sm bg-white/10 border border-white/10 rounded-lg hover:bg-white/20 transition-colors">
                    Change Password
                  </button>
                </div>
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">Two-Factor Authentication</h2>
                  <p className="text-sm text-white/60 mb-4">Add an extra layer of security to your account.</p>
                  <button className="px-4 py-2 text-sm bg-white/10 border border-white/10 rounded-lg hover:bg-white/20 transition-colors">
                    Enable 2FA
                  </button>
                </div>
              </div>
            )}

            {activeSection === 'notifications' && (
              <div className="space-y-6">
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">Email Notifications</h2>
                  <div className="space-y-3">
                    {['Trade confirmations', 'Order fills', 'Price alerts', 'Security alerts', 'Newsletter'].map((item) => (
                      <label key={item} className="flex items-center justify-between">
                        <span className="text-sm text-white/80">{item}</span>
                        <input type="checkbox" defaultChecked className="w-4 h-4 rounded bg-white/10 border-white/20" />
                      </label>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {activeSection === 'api-keys' && (
              <div className="space-y-6">
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">API Keys</h2>
                  <p className="text-sm text-white/60 mb-4">Manage API keys for programmatic access to your account.</p>
                  <button className="px-4 py-2 text-sm bg-white/10 border border-white/10 rounded-lg hover:bg-white/20 transition-colors">
                    Create API Key
                  </button>
                </div>
              </div>
            )}

            {activeSection === 'billing' && (
              <div className="space-y-6">
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">Billing</h2>
                  <p className="text-sm text-white/60">No billing information required for demo accounts.</p>
                </div>
              </div>
            )}

            {activeSection === 'help' && (
              <div className="space-y-6">
                <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-4">
                  <h2 className="text-sm font-medium mb-4">Help & Support</h2>
                  <p className="text-sm text-white/60 mb-4">Need help? Check out our documentation or contact support.</p>
                  <div className="flex gap-3">
                    <button className="px-4 py-2 text-sm bg-white/10 border border-white/10 rounded-lg hover:bg-white/20 transition-colors">
                      Documentation
                    </button>
                    <button className="px-4 py-2 text-sm bg-white/10 border border-white/10 rounded-lg hover:bg-white/20 transition-colors">
                      Contact Support
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
