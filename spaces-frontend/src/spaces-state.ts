import { createStore } from "solid-js/store";
import type { UserStateWithID, WsMsg } from "./types";
import crab from "crabuccino";

export function createSpacesStore() {
  const [users, setUsers] = createStore<UserStateWithID[]>([]);

  function handleUpdate(msg: WsMsg) {
    switch (msg.type) {
      case "all_users_state":
        setUsers(() => msg.users);
        if (users.length !== msg.users.length) {
          crab.panic("did not reset users store on all_users_state msg");
        }
        return;
      case "new_user":
        // don't add ourselves
        if (users.find((user) => user.id === msg.id)) return;
        setUsers(users.length, {
          id: msg.id,
          state: {
            mouse_pos: {
              // Use a random position to make it look like
              // new users pop up in a random position
              x: Math.random(),
              y: Math.random(),
            },
          },
        });
        return;
      case "disconnect":
        setUsers((prev) => prev.filter((user) => user.id !== msg.id));
        return;
      case "mouse_move":
        setUsers((user) => user.id === msg.id, "state", "mouse_pos", msg.mouse);
        return;
      case "location_found":
        setUsers((user) => user.id === msg.id, "state", "location", msg.loc);
        return;
      case "connected":
      case "error":
    }
  }
  return {
    users,
    handleUpdate,
  };
}
