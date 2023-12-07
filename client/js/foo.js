// Test web socket
const genRanHex = size => [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join('');

let websocket;
export function createWebSocket() {
    websocket = new WebSocket("ws://localhost:3000");
    websocket.onopen = (event) => {
        // First thing to do is to send this client's UUID
        websocket.send(genRanHex(8));
    };
}

export function sendPosition(x, y) {
    if (websocket.readyState !== WebSocket.OPEN) {
        return;
    }

    websocket.send(JSON.stringify({ x, y }));
}
