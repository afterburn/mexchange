import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Routes, Route } from 'react-router-dom'
import './index.css'
import App from './App.tsx'
import SignIn from './pages/SignIn.tsx'
import SignUp from './pages/SignUp.tsx'
import Portfolio from './pages/Portfolio.tsx'
import History from './pages/History.tsx'
import Account from './pages/Account.tsx'
import OrderDetail from './pages/OrderDetail.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<App />} />
        <Route path="/signin" element={<SignIn />} />
        <Route path="/signup" element={<SignUp />} />
        <Route path="/portfolio" element={<Portfolio />} />
        <Route path="/history" element={<History />} />
        <Route path="/account" element={<Account />} />
        <Route path="/account/:section" element={<Account />} />
        <Route path="/orders/:orderId" element={<OrderDetail />} />
      </Routes>
    </BrowserRouter>
  </StrictMode>,
)
