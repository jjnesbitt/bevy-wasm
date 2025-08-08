// The websocket used to communicate to the server
let websocket;

// Array of messages as strings
let latestMessage;

export function createWebSocket() {
    websocket = new WebSocket("ws://localhost:3000");
    websocket.onmessage = (event) => {
        latestMessage = event.data
    };
}

export function readLatestMessage() {
    return latestMessage;
}

export function sendPosition(x, y) {
    if (websocket.readyState !== WebSocket.OPEN) {
        return;
    }

    websocket.send(JSON.stringify({ x, y }));
}
