import { cacheExchange, createClient, fetchExchange, subscriptionExchange } from 'urql';
import { createClient as createWSClient } from 'graphql-ws';

// WebSocket client — Vite proxies ws://localhost:3000/graphql → ws://localhost:8080/graphql.
// async-graphql uses the graphql-ws subprotocol (not the legacy subscriptions-transport-ws).
const wsClient = createWSClient({
  url: `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/graphql`,
  retryAttempts: 5,
});

export const client = createClient({
  url: '/graphql',
  exchanges: [
    cacheExchange,
    fetchExchange,
    subscriptionExchange({
      forwardSubscription: (request) => ({
        subscribe: (sink) => ({
          unsubscribe: wsClient.subscribe(
            { query: request.query!, variables: request.variables },
            sink,
          ),
        }),
      }),
    }),
  ],
});
