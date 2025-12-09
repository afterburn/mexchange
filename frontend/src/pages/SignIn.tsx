import { useState, type FormEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Form, Input, Button } from '../components/core/Form';
import { useAuthStore } from '../stores/authStore';
import logoSvg from '../assets/logo.svg';

export default function SignIn() {
  const navigate = useNavigate();
  const { requestOtp, verifyOtp, isLoading, error, clearError } = useAuthStore();

  const [email, setEmail] = useState('');
  const [code, setCode] = useState('');
  const [step, setStep] = useState<'email' | 'otp'>('email');

  const handleRequestOtp = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    const success = await requestOtp(email);
    if (success) {
      setStep('otp');
    }
  };

  const handleVerifyOtp = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    const success = await verifyOtp(email, code);
    if (success) {
      navigate('/');
    }
  };

  return (
    <div className="min-h-screen bg-black flex flex-col items-center justify-center px-4">
      <Link to="/" className="mb-8">
        <img src={logoSvg} alt="mExchange" className="h-8" />
      </Link>

      {step === 'email' ? (
        <Form
          title="Sign in"
          subtitle="Enter your email to receive a verification code"
          onSubmit={handleRequestOtp}
          footer={
            <>
              Don't have an account?{' '}
              <Link to="/signup" className="text-white hover:underline">
                Sign up
              </Link>
            </>
          }
        >
          <Input
            label="Email"
            type="email"
            placeholder="you@example.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            error={error || undefined}
            required
          />
          <Button type="submit" loading={isLoading} className="mt-2">
            Send code
          </Button>
        </Form>
      ) : (
        <Form
          title="Enter verification code"
          subtitle={`We sent a code to ${email}`}
          onSubmit={handleVerifyOtp}
          footer={
            <button
              type="button"
              onClick={() => setStep('email')}
              className="text-white hover:underline"
            >
              Use different email
            </button>
          }
        >
          <Input
            label="Verification code"
            type="text"
            placeholder="000000"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            error={error || undefined}
            maxLength={6}
            required
          />
          <Button type="submit" loading={isLoading} className="mt-2">
            Verify
          </Button>
        </Form>
      )}
    </div>
  );
}
