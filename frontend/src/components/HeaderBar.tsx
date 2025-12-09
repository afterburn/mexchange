import { Link, useNavigate } from 'react-router-dom';
import { User, Wallet, History, UserCog, LogOut } from 'lucide-react';
import { useAuthStore } from '../stores/authStore';
import Dropdown, { DropdownItem, DropdownLabel, DropdownDivider } from './Dropdown';
import logoSvg from '../assets/logo.svg';

interface HeaderBarProps {
  onLogout?: () => void;
}

export default function HeaderBar({ onLogout }: HeaderBarProps) {
  const navigate = useNavigate();
  const { user, logout } = useAuthStore();

  const handleLogout = async () => {
    await logout();
    onLogout?.();
  };

  return (
    <div className="flex items-center justify-between px-4 h-11 border-b border-white/10 shrink-0">
      <div className="flex items-center gap-6">
        <Link to="/">
          <img src={logoSvg} alt="mExchange" className="h-5" />
        </Link>
        <div className="w-px h-4 bg-white/20" />
        <Link to="/" className="text-xs text-white/60 hover:text-white transition-colors">
          Trade
        </Link>
      </div>
      {user ? (
        <Dropdown
          trigger={
            <div className="w-7 h-7 rounded-full bg-white/10 flex items-center justify-center hover:bg-white/20 transition-colors">
              <User size={14} className="text-white/60" />
            </div>
          }
        >
          <DropdownLabel>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-full bg-white/10 flex items-center justify-center shrink-0">
                <User size={16} className="text-white/60" />
              </div>
              <div className="min-w-0">
                <div className="text-white text-sm truncate">{user.email}</div>
                <div className="text-white/40 text-[10px] font-mono truncate">{user.id}</div>
              </div>
            </div>
          </DropdownLabel>
          <DropdownItem onClick={() => navigate('/account')}>
            <UserCog size={14} />
            Account
          </DropdownItem>
          <DropdownItem onClick={() => navigate('/portfolio')}>
            <Wallet size={14} />
            Portfolio
          </DropdownItem>
          <DropdownItem onClick={() => navigate('/history')}>
            <History size={14} />
            History
          </DropdownItem>
          <DropdownDivider />
          <DropdownItem onClick={handleLogout}>
            <LogOut size={14} />
            Sign out
          </DropdownItem>
        </Dropdown>
      ) : (
        <div className="flex items-center gap-3">
          <Link to="/signin" className="text-xs text-white/60 hover:text-white transition-colors">
            Sign in
          </Link>
          <Link to="/signup" className="px-3 py-1 text-xs bg-white text-black rounded hover:bg-white/90 transition-colors">
            Sign up
          </Link>
        </div>
      )}
    </div>
  );
}
