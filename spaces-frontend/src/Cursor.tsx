import type { UserState } from "./types";
import { Cursor as CursorIcon, MapPin, Shield } from "phosphor-solid";
import "./cursor.css";
import { Show } from "solid-js";

const LOC_OFFSET = "25px";

export function Cursor(props: { user: UserState }) {
  return (
    <div
      class="cursor"
      style={{
        left: props.user.mouse_pos.x * 100 + "%",
        top: props.user.mouse_pos.y * 100 + "%",
      }}
    >
      <CursorIcon size={24} color="#ffffff" weight="bold" />
      <div
        class="cursor-loc"
        style={{
          top: "-3px",
          left: props.user.mouse_pos.x < 0.5 ? LOC_OFFSET : undefined,
          right: props.user.mouse_pos.x >= 0.5 ? LOC_OFFSET : undefined,
        }}
      >
        <MapPin />
        <Show when={props.user.location} fallback="Unknown">
          {props.user.location?.city}, {props.user.location?.country}
          <Show when={props.user.location?.using_vpn}>
            <span class="vpn">
              <Shield /> VPN
            </span>
          </Show>
        </Show>
      </div>
    </div>
  );
}
