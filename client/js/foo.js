// The websocket used to communicate to the server
let websocket;

// Array of messages as strings
let bufferedMessages = [];

export function createWebSocket() {
    websocket = new WebSocket("ws://localhost:3000");
    websocket.onmessage = (event) => {
        // Push string to array
        bufferedMessages.push(event.data);
    };
}

export function readMessages() {
    const messages = bufferedMessages;
    bufferedMessages = [];

    return messages;
}

export function sendPosition(x, y) {
    if (websocket.readyState !== WebSocket.OPEN) {
        return;
    }

    websocket.send(JSON.stringify({ x, y }));
}
