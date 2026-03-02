import { useState } from 'react';
import { useMutation } from 'urql';
import {
  CANCEL_ORDER_MUTATION, PLACE_ORDER_MUTATION,
  type CancelOrderData, type CancelOrderVars,
  type PlaceOrderData, type PlaceOrderVars,
} from '../queries';

function newCommandId() {
  return crypto.randomUUID();
}

// ── Place Order ───────────────────────────────────────────────────────────────

function PlaceOrderForm() {
  const [side, setSide] = useState<'BID' | 'ASK'>('BID');
  const [orderId, setOrderId]   = useState('');
  const [userId, setUserId]     = useState('1');
  const [price, setPrice]       = useState('');
  const [quantity, setQuantity] = useState('');

  const [result, executePlaceOrder] = useMutation<PlaceOrderData, PlaceOrderVars>(PLACE_ORDER_MUTATION);

  const [lastResult, setLastResult] = useState<string | null>(null);
  const [lastError,  setLastError]  = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLastResult(null);
    setLastError(null);

    const res = await executePlaceOrder({
      commandId: newCommandId(),
      orderId:   orderId.trim(),
      userId:    userId.trim(),
      side,
      price:     Number(price),
      quantity:  Number(quantity),
    });

    if (res.error) {
      setLastError(res.error.message);
    } else {
      const p = res.data?.placeOrder;
      setLastResult(
        `Accepted at seq ${p?.acceptedSeq}\n` +
        `Fills: ${p?.fills.length ?? 0}  ·  Remaining qty: ${p?.remainingQty}\n` +
        `Idempotent retry: ${!p?.inserted}` +
        (p?.fills.length
          ? '\n\nFills:\n' + p.fills.map(f =>
              `  maker #${f.makerOrderId}  ←  taker #${f.takerOrderId}  qty ${f.quantity} @ ${f.price}`
            ).join('\n')
          : ''),
      );
    }
  };

  return (
    <div className="card">
      <h2>Place Limit Order</h2>
      <form onSubmit={submit}>
        <div className="field mt-16" style={{ marginBottom: 16 }}>
          <label>Side</label>
          <div className="side-toggle">
            <button type="button" className={`bid ${side === 'BID' ? 'active' : ''}`}
              onClick={() => setSide('BID')}>BID (buy)</button>
            <button type="button" className={`ask ${side === 'ASK' ? 'active' : ''}`}
              onClick={() => setSide('ASK')}>ASK (sell)</button>
          </div>
        </div>

        <div className="form-grid">
          <div className="field">
            <label>Order ID</label>
            <input type="number" min="1" placeholder="e.g. 42" value={orderId}
              onChange={e => setOrderId(e.target.value)} required />
          </div>
          <div className="field">
            <label>User ID</label>
            <input type="number" min="1" placeholder="e.g. 1" value={userId}
              onChange={e => setUserId(e.target.value)} required />
          </div>
          <div className="field">
            <label>Price (ticks)</label>
            <input type="number" min="1" placeholder="e.g. 100" value={price}
              onChange={e => setPrice(e.target.value)} required />
          </div>
          <div className="field">
            <label>Quantity (lots)</label>
            <input type="number" min="1" placeholder="e.g. 5" value={quantity}
              onChange={e => setQuantity(e.target.value)} required />
          </div>
        </div>

        <button
          type="submit"
          className={`btn ${side === 'BID' ? 'btn-bid' : 'btn-ask'} mt-16`}
          disabled={result.fetching}
        >
          {result.fetching ? 'Submitting…' : `Place ${side}`}
        </button>
      </form>

      {lastResult && <pre className="result-box result-ok">{lastResult}</pre>}
      {lastError  && <pre className="result-box result-err">{lastError}</pre>}
    </div>
  );
}

// ── Cancel Order ──────────────────────────────────────────────────────────────

function CancelOrderForm() {
  const [orderId, setOrderId] = useState('');
  const [result, executeCancelOrder] = useMutation<CancelOrderData, CancelOrderVars>(CANCEL_ORDER_MUTATION);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [lastError,  setLastError]  = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLastResult(null);
    setLastError(null);

    const res = await executeCancelOrder({
      commandId: newCommandId(),
      orderId:   orderId.trim(),
    });

    if (res.error) {
      setLastError(res.error.message);
    } else {
      const c = res.data?.cancelOrder;
      setLastResult(
        `Canceled at seq ${c?.eventSeq}\nIdempotent retry: ${!c?.inserted}`,
      );
    }
  };

  return (
    <div className="card mt-16">
      <h2>Cancel Resting Order</h2>
      <form onSubmit={submit} className="flex gap-8 mt-16" style={{ alignItems: 'flex-end' }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Order ID to cancel</label>
          <input type="number" min="1" placeholder="e.g. 42" value={orderId}
            onChange={e => setOrderId(e.target.value)} required />
        </div>
        <button type="submit" className="btn btn-ghost" disabled={result.fetching}>
          {result.fetching ? 'Canceling…' : 'Cancel Order'}
        </button>
      </form>

      {lastResult && <pre className="result-box result-ok">{lastResult}</pre>}
      {lastError  && <pre className="result-box result-err">{lastError}</pre>}
    </div>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────────

export function TradeForm() {
  return (
    <div className="page">
      <h1>Trade</h1>
      <p className="muted" style={{ marginBottom: 20 }}>
        Each submission generates a UUID <code>commandId</code> automatically — retrying
        the same form with the same order ID is idempotent.
      </p>
      <PlaceOrderForm />
      <CancelOrderForm />
    </div>
  );
}
