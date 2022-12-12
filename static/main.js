let log = console.log;

let nome = prompt("Enter your name");

const wsUri = ((window.location.protocol == "https:" && "wss://") || "ws://") +
  window.location.host +
  "/ws";
conn = new WebSocket(wsUri);

log("Connecting...");

conn.onopen = function () {
  log("Connected.");
};

conn.onmessage = function (e) {
  log("Received: " + e.data);
  let msg = JSON.parse(e.data);
  //let str = `${msg.name} (${msg.uid}): ${msg.message}`;
  document.getElementById("log").appendChild(createMsg(msg));
};

conn.onclose = function () {
  log("Disconnected.");
  conn = null;
};

function createMsg(message) {
  const msg = document.createElement("div");
  msg.textContent = message.message;
  msg.classList.add("msg");
  const name = document.createElement("div");
  name.textContent = `${message.name} (${message.uid})`;
  name.classList.add("nom");
  const s = document.createElement("div");
  s.appendChild(name);
  s.appendChild(msg);
  s.classList.add("bubble");
  return s;
}

function send() {
  conn.send(
    JSON.stringify({
      name: nome,
      message: document.getElementById("input").value,
    }),
  );
  document.getElementById("input").value = "";
}

document.getElementById("btn")?.addEventListener("click", send);

document.getElementById("input")?.addEventListener("keypress", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    send();
  }
});
