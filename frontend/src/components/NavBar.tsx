import { NavLink } from 'react-router-dom';

export function NavBar() {
  return (
    <nav>
      <NavLink to="/" className="nav-brand">
        raft<span>book</span>
      </NavLink>
      <NavLink to="/"        end>Dashboard</NavLink>
      <NavLink to="/trade"     >Trade</NavLink>
      <NavLink to="/events"    >Live Events</NavLink>
      <NavLink to="/explorer"  >Explorer</NavLink>
    </nav>
  );
}
