# Snake game

https://github.com/phantie/snake/assets/43893037/2cdbeae7-6fd6-4bb2-97e4-82d1cfe65562

### Modes
Frontend self-contains singleplayer mode.\
Websocket connection to backend enables multiplayer mode.

### Stack
k8s, yew, nix, axum

Deployed on kubernetes. Dev environment and docker images built by Nix. Compiled frontend artifacts, served by dedicated cache-optimized http server.

### Backend
Performant concurrent event handling on asynchronous multi-threaded tokio runtime, similar to Erlang's light-weight processes.

### Frontend
Based on SPA application framework similar to React. Custom theme support (click right top corner).

### Messages
Frontend and backend reuse message schemas, compile-time checked. Request/response model over Websockets implemented for frontend and backend. Reliable communication provided by request/response model (acknowledgements), idempotency and (potentially) request retries.

### Development guide
Refer to dev.md

### Optimizations
#### Space efficient ser/de for snake form
Reduces required space for snake form ser/de by encoding directions intead of positions after initial position. \
Old payload: {pos1 pos2 pos3} \
New payload: {pos1 dir1 dir2} \
Position as json is {"x":x,"y":y} taking 11 + 2 * (1 to 5 (1 for possible minus sign)) bytes = 13 to 21 bytes. Encoded position as a direction takes 2 bits. Resulting in space efficiency per position increased by up to (max(13, 21) * 8) / 2 = 84 times.

### Room for improvement
- State reconciliation using partial update, instead of pushing full state to client on server update. Goal: smaller payloads.
- Compress messages, for example, leaving field names out of payloads. Goal: smaller payloads.
- Push updates only for possibly visible to client objects. Goal: smaller payloads.
- Root frontend component refactoring. Goal: reduce technical debt.
