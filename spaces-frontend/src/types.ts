type TaggedUnion<K extends PropertyKey, T extends object> = {
  [P in keyof T]: { [Q in K]: P } & T[P] extends infer U
    ? { [Q in keyof U]: U[Q] }
    : never;
}[keyof T];

export type UnionVariant<T, K extends PropertyKey, V> =
  T extends Record<K, V> ? T : never;

export type WithoutVariant<T, K extends PropertyKey, V> = Exclude<
  T,
  UnionVariant<T, K, V>
>;

export type UserID = number;

export type WsMsg = TaggedUnion<
  "type",
  {
    all_users_state: { users: UserStateWithID[] };
    new_user: { id: UserID };
    connected: { id: UserID };
    disconnect: { id: UserID };
    mouse_move: { id: UserID; mouse: MouseState };
    client_mouse_move: { mouse: MouseState };
    location_found: { id: UserID; loc: Location };
    error: { msg: string };
  }
>;

export type UserState = {
  location?: Location;
  mouse_pos: MouseState;
};

export type UserStateWithID = {
  id: UserID;
  state: UserState;
};

export type MouseState = {
  x: number;
  y: number;
};

export type Location = {
  country: string;
  region: string;
  city: string;
  using_vpn: boolean;
};

export type ConnectionStatus =
  "connected" | "disconnected" | "error" | "connecting";
