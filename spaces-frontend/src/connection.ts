import { crab, type Result } from "crabuccino";
import type { MouseState, UnionVariant, WithoutVariant, WsMsg } from "./types";
import { decode, encode } from "@msgpack/msgpack";

const WS_URL = (() => {
  if (import.meta.env.DEV) {
    return "http://localhost:8923/ws";
  }
  const prod = import.meta.env.VITE_WS_URL;
  if (!prod) {
    throw new Error("VITE_WS_URL is not set");
  }
  return prod as string;
})();

const CLIENT_SECRET: string = import.meta.env.VITE_CLIENT_SECRET;

function isMobileDevice(): boolean {
  return (
    "ontouchstart" in window ||
    navigator.maxTouchPoints > 0 ||
    navigator.maxTouchPoints > 0
  );
}

export type ConnectionHandlers = {
  onClose: (e: CloseEvent) => void;
  onError: (e: Error) => void;
  onConnect: (id: number) => void;
  onMsg: (msg: WithoutVariant<WsMsg, "type", "error">) => void;
};

export class SpacesConnection {
  private id: number | null = null;
  private conn: WebSocket;

  constructor(handlers: ConnectionHandlers) {
    const conn = new WebSocket(
      `${WS_URL}?secret=${CLIENT_SECRET}&is_mobile=${isMobileDevice()}`,
    );
    this.conn = conn;
    conn.binaryType = "arraybuffer";
    conn.addEventListener("close", (e) => {
      handlers.onClose(e);
    });
    conn.addEventListener("error", (e) => {
      handlers.onError(crab.unknownToError(e));
    });
    conn.addEventListener("message", (msg) => {
      try {
        function parse(msg: MessageEvent<unknown>) {
          if (msg.data instanceof ArrayBuffer) {
            return safeDecode(msg.data) as Result<WsMsg, Error>;
          } else if (typeof msg.data == "string") {
            return safeJsonParse(msg.data) as Result<WsMsg, Error>;
          } else {
            throw new Error("unknown ws frame data");
          }
        }
        parse(msg).match((wsMsg: WsMsg) => {
          if (wsMsg.type === "error") {
            handlers.onError(new Error(wsMsg.msg));
          } else if (wsMsg.type === "connected") {
            this.id = wsMsg.id;
            handlers.onConnect(this.id);
          } else {
            handlers.onMsg(wsMsg);
          }
        }, handlers.onError);
      } catch (e) {
        handlers.onError(crab.unknownToError(e));
      }
    });
  }

  public send_mouse_update(mouse: MouseState): Result<void, Error> {
    if (this.id === null) {
      return crab.Err(new Error("connection ID not yet received"));
    }

    const msg: UnionVariant<WsMsg, "type", "client_mouse_move"> = {
      type: "client_mouse_move",
      mouse,
    };

    if (BINARY) {
      return crab.try(() => this.conn.send(encode(msg)), crab.unknownToError);
    } else {
      return crab.try(
        () => this.conn.send(JSON.stringify(msg)),
        crab.unknownToError,
      );
    }
  }

  public close(code?: number, reason?: string) {
    this.conn.close(code, reason);
  }
}

const BINARY = true;
const safeDecode = crab.makeSafe(decode, crab.unknownToError);
const safeJsonParse = crab.makeSafe(JSON.parse, crab.unknownToError);
