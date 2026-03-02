import { createGraphiQLFetcher } from '@graphiql/toolkit';
import { GraphiQL } from 'graphiql';
import 'graphiql/style.css';
import { EXPLORER_DEFAULT_QUERY } from '../queries';

// The fetcher handles both HTTP queries/mutations and WebSocket subscriptions.
const fetcher = createGraphiQLFetcher({
  url: '/graphql',
  subscriptionUrl: `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/graphql`,
});

export function Explorer() {
  return (
    <div className="explorer-wrap">
      <GraphiQL
        fetcher={fetcher}
        defaultQuery={EXPLORER_DEFAULT_QUERY}
        defaultHeaders={JSON.stringify({ 'Content-Type': 'application/json' })}
      />
    </div>
  );
}
