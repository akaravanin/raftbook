import { Route, Routes } from 'react-router-dom';
import { NavBar } from './components/NavBar';
import { Dashboard }   from './pages/Dashboard';
import { EventStream } from './pages/EventStream';
import { Explorer }    from './pages/Explorer';
import { TradeForm }   from './pages/TradeForm';

export function App() {
  return (
    <>
      <NavBar />
      <Routes>
        <Route path="/"         element={<Dashboard />}   />
        <Route path="/trade"    element={<TradeForm />}   />
        <Route path="/events"   element={<EventStream />} />
        <Route path="/explorer" element={<Explorer />}    />
      </Routes>
    </>
  );
}
