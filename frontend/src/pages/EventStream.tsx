import { useState } from 'react';
import { useSubscription } from 'urql';
import {
  EVENT_STREAM_SUBSCRIPTION,
  type EventRecord, type EventStreamData, type EventStreamVars,
} from '../queries';

function accumulate(existing: EventRecord[] = [], response: EventStreamData): EventRecord[] {
  // Keep at most 200 events in memory; drop the oldest when over limit.
  const next = [...existing, response.eventStream];
  return next.length > 200 ? next.slice(next.length - 200) : next;
}

function eventLabel(r: EventRecord) {
  switch (r.event.__typename) {
    case 'GqlOrderAccepted': return { tag: 'accepted', cls: 'tag-accepted', detail: `order #${r.event.orderId}` };
    case 'GqlTradeExecuted': return {
      tag: 'trade', cls: 'tag-trade',
      detail: `${r.event.quantity} @ ${r.event.price}  (maker #${r.event.makerOrderId})`,
    };
    case 'GqlOrderCanceled': return { tag: 'canceled', cls: 'tag-canceled', detail: `order #${r.event.orderId}` };
  }
}

export function EventStream() {
  const [fromSeq, setFromSeq] = useState(0);
  const [paused, setPaused] = useState(false);

  const [{ data: events, fetching, error }] = useSubscription<
    EventRecord[], EventStreamData, EventStreamVars
  >(
    { query: EVENT_STREAM_SUBSCRIPTION, variables: { fromSeq }, pause: paused },
    accumulate,
  );

  const rows = [...(events ?? [])].reverse(); // newest at top

  return (
    <div className="page-full" style={{ padding: '28px 32px', display: 'flex', flexDirection: 'column' }}>
      <div className="stream-header">
        <h1 style={{ margin: 0 }}>Live Event Stream</h1>
        <div className="flex gap-8">
          <span className={`pill ${error ? 'pill-err' : fetching ? 'pill-ok' : 'pill-err'}`}>
            <span className="dot" />
            {error ? 'error' : paused ? 'paused' : 'live'}
          </span>
          <button className="btn btn-ghost" onClick={() => setPaused(p => !p)}>
            {paused ? 'Resume' : 'Pause'}
          </button>
          <button className="btn btn-ghost" onClick={() => {
            // Re-subscribe from latest sequence to reset the buffer.
            const latest = events?.at(-1)?.seq;
            if (latest !== undefined) setFromSeq(latest + 1);
          }}>
            Clear
          </button>
        </div>
      </div>

      {error && (
        <div className="result-box result-err" style={{ marginBottom: 16 }}>
          {error.message}
        </div>
      )}

      <div className="stream-container">
        {rows.length === 0 ? (
          <p className="stream-empty">
            {paused ? 'Subscription paused.' : 'Waiting for events… place an order to see activity.'}
          </p>
        ) : (
          <table className="event-table">
            <thead>
              <tr>
                <th>Seq</th>
                <th>Type</th>
                <th>Detail</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const { tag, cls, detail } = eventLabel(r);
                return (
                  <tr key={r.seq}>
                    <td className="mono muted">{r.seq}</td>
                    <td><span className={`tag ${cls}`}>{tag}</span></td>
                    <td className="mono">{detail}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
