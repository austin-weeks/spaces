import type { UserState } from "./types";
import {
  Cursor as CursorIcon,
  HandPointing,
  MapPin,
  Shield,
} from "phosphor-solid";
import "./cursor.css";
import { Show } from "solid-js";

const LOC_OFFSET = "25px";

export function Cursor(props: { user: UserState }) {
  const orRandom = { x: Math.random(), y: Math.random() };
  const x = () => props.user.mouse_pos?.x ?? orRandom.x;
  const y = () => props.user.mouse_pos?.y ?? orRandom.y;

  return (
    <div
      class="cursor"
      style={{
        left: x() * 100 + "%",
        top: y() * 100 + "%",
      }}
    >
      <Show
        when={props.user.is_mobile}
        fallback={<CursorIcon size={24} color="#ffffff" weight="bold" />}
      >
        <HandPointing size={24} color="#ffffff" weight="bold" />
      </Show>
      <div
        class="cursor-loc"
        style={{
          top: "-3px",
          left: x() < 0.5 ? LOC_OFFSET : undefined,
          right: x() >= 0.5 ? LOC_OFFSET : undefined,
        }}
      >
        <MapPin />
        <Show when={props.user.location} fallback="Unknown">
          {(loc) => (
            <>
              {loc().city}, {loc().country}
              <Show when={props.user.location?.using_vpn}>
                <span class="vpn">
                  <Shield /> VPN
                </span>
              </Show>
            </>
          )}
        </Show>
      </div>
    </div>
  );
}
