import type { LogServerMessage } from '../../server/eventbus-protocol.ts';
import { sendLiveMessage, subscribeLiveMessages } from '../lib/live-client.ts';
import { initialLogState, updateLog, type LogAction, type LogUpdate } from './session-log-model.ts';

export function createSessionLogController({ sessionId, send = sendLiveMessage, subscribe = subscribeLiveMessages, onChange }: {
  sessionId: string; send?: typeof sendLiveMessage; subscribe?: typeof subscribeLiveMessages;
  onChange: (update: LogUpdate) => void;
}) {
  let state = initialLogState(sessionId);
  let unsubscribe: (() => void) | undefined;
  function dispatch(action: LogAction) {
    const update = updateLog(state, action);
    state = update.state;
    if (update.send) send(update.send);
    onChange(update);
  }
  return {
    dispatch,
    open() {
      if (unsubscribe) return;
      unsubscribe = subscribe((type, message) => {
        if (type === 'hello') dispatch({ type: 'hello' });
        else if (message.session_id === sessionId) {
          const log = message as LogServerMessage;
          if (log.type === 'log.chunk') dispatch({ type: 'chunk', message: log });
          else if (log.type === 'log.end') dispatch({ type: 'end', message: log });
          else if (log.type === 'log.rejected') dispatch({ type: 'rejected', message: log });
        }
      });
      dispatch({ type: 'open' });
    },
    close() { unsubscribe?.(); unsubscribe = undefined; dispatch({ type: 'close' }); },
  };
}
