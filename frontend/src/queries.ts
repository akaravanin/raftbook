// ── TypeScript domain types ───────────────────────────────────────────────────

export interface GqlOrderAccepted {
  __typename: 'GqlOrderAccepted';
  orderId: string;
}
export interface GqlTradeExecuted {
  __typename: 'GqlTradeExecuted';
  makerOrderId: string;
  takerOrderId: string;
  price: number;
  quantity: number;
}
export interface GqlOrderCanceled {
  __typename: 'GqlOrderCanceled';
  orderId: string;
}
export type GqlEvent = GqlOrderAccepted | GqlTradeExecuted | GqlOrderCanceled;

export interface EventRecord {
  seq: number;
  event: GqlEvent;
}

export interface Fill {
  makerOrderId: string;
  takerOrderId: string;
  price: number;
  quantity: number;
}

// ── Queries ───────────────────────────────────────────────────────────────────

export const HEALTH_QUERY = `
  query Health {
    health
  }
`;

export const EVENTS_QUERY = `
  query Events($fromSeq: Int!) {
    events(fromSeq: $fromSeq) {
      seq
      event {
        __typename
        ... on GqlOrderAccepted { orderId }
        ... on GqlTradeExecuted {
          makerOrderId
          takerOrderId
          price
          quantity
        }
        ... on GqlOrderCanceled { orderId }
      }
    }
  }
`;

export interface EventsData    { events: EventRecord[] }
export interface EventsVars    { fromSeq: number }
export interface HealthData    { health: string }

// ── Mutations ─────────────────────────────────────────────────────────────────

export const PLACE_ORDER_MUTATION = `
  mutation PlaceOrder(
    $commandId: String!
    $orderId:   ID!
    $userId:    ID!
    $side:      GqlSide!
    $price:     Int!
    $quantity:  Int!
  ) {
    placeOrder(
      commandId: $commandId
      orderId:   $orderId
      userId:    $userId
      side:      $side
      price:     $price
      quantity:  $quantity
    ) {
      acceptedSeq
      fills { makerOrderId takerOrderId price quantity }
      remainingQty
      inserted
    }
  }
`;

export const CANCEL_ORDER_MUTATION = `
  mutation CancelOrder($commandId: String!, $orderId: ID!) {
    cancelOrder(commandId: $commandId, orderId: $orderId) {
      eventSeq
      inserted
    }
  }
`;

export interface PlaceOrderVars {
  commandId: string;
  orderId:   string;
  userId:    string;
  side:      'BID' | 'ASK';
  price:     number;
  quantity:  number;
}
export interface PlaceOrderData {
  placeOrder: {
    acceptedSeq:   number;
    fills:         Fill[];
    remainingQty:  number;
    inserted:      boolean;
  };
}
export interface CancelOrderVars { commandId: string; orderId: string }
export interface CancelOrderData { cancelOrder: { eventSeq: number; inserted: boolean } }

// ── Subscriptions ─────────────────────────────────────────────────────────────

export const EVENT_STREAM_SUBSCRIPTION = `
  subscription EventStream($fromSeq: Int) {
    eventStream(fromSeq: $fromSeq) {
      seq
      event {
        __typename
        ... on GqlOrderAccepted { orderId }
        ... on GqlTradeExecuted {
          makerOrderId
          takerOrderId
          price
          quantity
        }
        ... on GqlOrderCanceled { orderId }
      }
    }
  }
`;

export interface EventStreamData { eventStream: EventRecord }
export interface EventStreamVars { fromSeq?: number }

// ── GraphiQL example queries (shown in the Explorer page) ─────────────────────

export const EXPLORER_DEFAULT_QUERY = `# RaftBook Exchange — GraphQL Explorer
#
# Seed data loaded on startup (idempotent):
#   Ask@102 qty10  Ask@104 qty5(partial)
#   Bid@99  qty5   (partial, after fill)
#   3× TradeExecuted, 1× OrderCanceled
#
# Keyboard shortcuts:  Ctrl+Enter run  ·  Shift+Alt+P prettify

# ── 1. Health check ───────────────────────────────────────────────────────────
query Health {
  health
}

# ── 2. Replay all seeded events ───────────────────────────────────────────────
# query RecentEvents {
#   events(fromSeq: 0) {
#     seq
#     event {
#       __typename
#       ... on GqlOrderAccepted { orderId }
#       ... on GqlTradeExecuted { makerOrderId takerOrderId price quantity }
#       ... on GqlOrderCanceled { orderId }
#     }
#   }
# }

# ── 3. Place a limit ask (new level above seed book) ─────────────────────────
# mutation PlaceAsk {
#   placeOrder(
#     commandId: "demo-ask-001"
#     orderId:   "100"
#     userId:    "99"
#     side:       ASK
#     price:     106
#     quantity:   8
#   ) {
#     acceptedSeq fills { price quantity } remainingQty inserted
#   }
# }

# ── 4. Place a crossing bid (hits seed ask@104) ───────────────────────────────
# mutation PlaceBid {
#   placeOrder(
#     commandId: "demo-bid-001"
#     orderId:   "101"
#     userId:    "99"
#     side:       BID
#     price:     105
#     quantity:   10
#   ) {
#     acceptedSeq fills { makerOrderId takerOrderId price quantity } remainingQty inserted
#   }
# }

# ── 5. Cancel a resting order ─────────────────────────────────────────────────
# mutation Cancel {
#   cancelOrder(commandId: "demo-cancel-001", orderId: "3") {
#     eventSeq inserted
#   }
# }

# ── 6. Live event stream (subscription — use WebSocket) ───────────────────────
# subscription LiveEvents {
#   eventStream(fromSeq: 0) {
#     seq
#     event {
#       __typename
#       ... on GqlTradeExecuted { price quantity }
#     }
#   }
# }
`;
