import { useQuery } from 'urql';
import {
  EVENTS_QUERY, HEALTH_QUERY,
  type EventRecord, type EventsData, type EventsVars, type HealthData,
} from '../queries';

function eventTag(record: EventRecord) {
  switch (record.event.__typename) {
    case 'GqlOrderAccepted': return <span className="tag tag-accepted">accepted</span>;
    case 'GqlTradeExecuted': return <span className="tag tag-trade">trade</span>;
    case 'GqlOrderCanceled': return <span className="tag tag-canceled">canceled</span>;
  }
}

function eventDetail(record: EventRecord) {
  const e = record.event;
  switch (e.__typename) {
    case 'GqlOrderAccepted':
      return <span className="muted">order #{e.orderId}</span>;
    case 'GqlTradeExecuted':
      return (
        <span>
          <span style={{ color: 'var(--bid)' }}>{e.quantity}</span>
          {' @ '}
          <span className="bold">{e.price}</span>
          <span className="muted"> (maker #{e.makerOrderId} ← taker #{e.takerOrderId})</span>
        </span>
      );
    case 'GqlOrderCanceled':
      return <span className="muted">order #{e.orderId}</span>;
  }
}

export function Dashboard() {
  const [{ data: healthData, fetching: healthFetching }] = useQuery<HealthData>({
    query: HEALTH_QUERY,
  });

  const [{ data: eventsData, fetching: eventsFetching }] = useQuery<EventsData, EventsVars>({
    query: EVENTS_QUERY,
    variables: { fromSeq: 0 },
  });

  const events = [...(eventsData?.events ?? [])].reverse().slice(0, 50);

  return (
    <div className="page">
      <h1>Dashboard</h1>

      {/* Health */}
      <div className="card">
        <h2>Engine Status</h2>
        {healthFetching ? (
          <span className="muted">checking…</span>
        ) : (
          <span className={`pill ${healthData?.health ? 'pill-ok' : 'pill-err'}`}>
            <span className="dot" />
            {healthData?.health ?? 'unreachable'}
          </span>
        )}
      </div>

      {/* Recent events */}
      <div className="card mt-16">
        <h2>Recent Events (newest first)</h2>
        {eventsFetching && <p className="muted">loading…</p>}
        {!eventsFetching && events.length === 0 && (
          <p className="muted">No events yet — place an order to see activity.</p>
        )}
        {events.length > 0 && (
          <table className="event-table">
            <thead>
              <tr>
                <th>Seq</th>
                <th>Type</th>
                <th>Detail</th>
              </tr>
            </thead>
            <tbody>
              {events.map((r) => (
                <tr key={r.seq}>
                  <td className="mono muted">{r.seq}</td>
                  <td>{eventTag(r)}</td>
                  <td className="mono">{eventDetail(r)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
