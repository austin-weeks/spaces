import { Match, Switch, type Accessor } from "solid-js";
import type { ConnectionStatus } from "./types";

import "./status-badge.css";

type ConnBadgeProps = {
  status: Accessor<ConnectionStatus>;
};
export default function ConnectionStatusBadge(props: ConnBadgeProps) {
  return (
    <div class="conn-status-badge">
      <Switch>
        <Match when={props.status() === "connected"}>
          <span class="conn-green" />
          <span class="conn-status">Connected</span>
        </Match>
        <Match when={props.status() === "disconnected"}>
          <span class="conn-red" />
          <span class="conn-status">Disconnected</span>
        </Match>
        <Match when={props.status() === "error"}>
          <span class="conn-red" />
          <span class="conn-status">Error</span>
        </Match>
        <Match when={props.status() === "connecting"}>
          <span class="conn-empty" />
          <span class="conn-status">Connecting...</span>
        </Match>
      </Switch>
    </div>
  );
}
