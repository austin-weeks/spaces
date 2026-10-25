import { SpacesConnection } from "./connection";
import type { ConnectionStatus } from "./types";
import ConnectionStatusBadge from "./ConnectionStatusBadge";
import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import { createSpacesStore } from "./spaces-state";
import { Cursor } from "./Cursor";

import "./cursors.css";
import { Check, Copy, PaperPlaneTilt } from "phosphor-solid";

const RECONNECT_SEC_TIMEOUTS = [1, 2, 5, 10, 15, 30, 60, 120, 300];

export default function Cursors() {
  const [connection, setConnection] = createSignal<SpacesConnection | null>(
    null,
  );
  const [connId, setConnID] = createSignal<number | null>(null);
  const { users, handleUpdate } = createSpacesStore();
  const [connStatus, setConnStatus] =
    createSignal<ConnectionStatus>("connecting");
  const [emptyDelayHit, setEmptyDelayHit] = createSignal(false);

  createEffect(() => {
    let disposed = false;
    let currentConn: SpacesConnection | undefined;
    let reconnectAttempt = 0;
    let retryTimeout: number | undefined;
    let emptyDelayTimeout: number | undefined;

    const abortHandler = (thisConn: SpacesConnection) =>
      disposed || currentConn !== thisConn;

    function connect() {
      setConnStatus("connecting");
      const thisConn = new SpacesConnection({
        onConnect: (id) => {
          if (abortHandler(thisConn)) {
            return;
          }
          reconnectAttempt = 0;
          setConnStatus("connected");
          setConnID(id);
          emptyDelayTimeout = setTimeout(() => setEmptyDelayHit(true), 2000);
        },
        onClose: () => {
          if (abortHandler(thisConn)) {
            return;
          }
          clearTimeout(emptyDelayTimeout);
          clearTimeout(retryTimeout);
          setConnection(null);
          setConnID(null);
          setConnStatus((prev) => (prev === "error" ? prev : "disconnected"));
          // Retry connect
          if (reconnectAttempt < RECONNECT_SEC_TIMEOUTS.length) {
            retryTimeout = setTimeout(
              connect,
              RECONNECT_SEC_TIMEOUTS[reconnectAttempt++] * 1000,
            );
          }
        },
        onError: (e) => {
          if (abortHandler(thisConn)) {
            return;
          }
          console.error("WS Error:", e);
          setConnStatus("error");
          thisConn.close();
        },
        onMsg: (msg) => !abortHandler(thisConn) && handleUpdate(msg),
      });
      currentConn = thisConn;
      setConnection(thisConn);
    }

    connect();

    onCleanup(() => {
      disposed = true;
      clearTimeout(emptyDelayTimeout);
      clearTimeout(retryTimeout);
      setConnection(null);
      setConnID(null);
      currentConn?.close();
    });
  });

  createEffect(() => {
    let mouseX = 0;
    let mouseY = 0;
    let lastSentX = mouseX;
    let lastSentY = mouseY;

    const debounce = () => {
      return setTimeout(
        () => {
          const conn = connection();
          if (conn && (mouseX !== lastSentX || mouseY !== lastSentY)) {
            conn.send_mouse_update({ x: mouseX, y: mouseY });
            lastSentX = mouseX;
            lastSentY = mouseY;
          }
          timeoutID = debounce();
        },
        500 + (Math.random() - 0.5) * 250,
      );
    };
    let timeoutID = debounce();

    function onMouseMove(ev: MouseEvent) {
      mouseX = ev.clientX / window.innerWidth;
      mouseY = ev.clientY / window.innerHeight;
    }
    document.addEventListener("mousemove", onMouseMove);

    onCleanup(() => {
      clearTimeout(timeoutID);
      document.removeEventListener("mousemove", onMouseMove);
    });
  });

  return (
    <>
      <Show when={users.length <= 1 && emptyDelayHit()}>
        <NooneHere />
      </Show>
      <For
        // Filter out ourselves
        each={users.filter((user) =>
          connId() === null ? true : user.id !== connId(),
        )}
      >
        {(user) => <Cursor user={user.state} />}
      </For>
      <ConnectionStatusBadge status={connStatus} />
    </>
  );
}

function NooneHere() {
  const [copied, setCopied] = createSignal(false);

  const shareAvailable = navigator.share !== undefined;
  function onClick() {
    if (shareAvailable) {
      navigator.share({
        title: "Spaces",
        text: "Join me on Spaces!",
        url: window.location.href,
      });
    } else {
      navigator.clipboard.writeText(window.location.href);
      setCopied(true);
    }
  }

  return (
    <div class="no-one-here">
      <div>
        <div>Looks like it's just you here...</div>
        <div>Invite your friends!</div>
      </div>
      <div class="copy-btn" onClick={onClick}>
        {shareAvailable ? (
          <PaperPlaneTilt />
        ) : (
          <Show when={!copied()} fallback={<Check />}>
            <Copy />
          </Show>
        )}
      </div>
    </div>
  );
}
