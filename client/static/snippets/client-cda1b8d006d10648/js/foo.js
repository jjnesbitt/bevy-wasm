// Test web socket
const exampleSocket = new WebSocket("ws://localhost:3000");
const sendMessage = (message) => {
    exampleSocket.send(`MESSAGE! ${Math.random()}`);
}

export function sendPosition(x, y) {
    if (!exampleSocket.OPEN) {
        return;
    }

    exampleSocket.send({x, y});
}

const genRanHex = size => [...Array(size)].map(() => Math.floor(Math.random() * 16).toString(16)).join('');
exampleSocket.onopen = (event) => {
    // First thing to do is to send this client's UUID
    exampleSocket.send(genRanHex(8));
};
