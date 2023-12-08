// Test web socket
const genRanHex = size => [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join('');

let websocket;
export function createWebSocket() {
    websocket = new WebSocket("ws://localhost:3000");
}

export function sendPosition(x, y) {
    if (websocket.readyState !== WebSocket.OPEN) {
        return;
    }

    websocket.send(JSON.stringify({ x, y }));
}
