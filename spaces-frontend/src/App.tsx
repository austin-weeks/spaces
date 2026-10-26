import { DottedGlowBackground } from "./Background";
import Cursors from "./Cursors";

import "./attribution.css";

export default function App() {
  return (
    <>
      <Cursors />
      <DottedGlowBackground
        opacity={0.9}
        color="#1e3a8a"
        gap={10}
        radius={2.5}
      />
      <div class="attribution">
        made by{" "}
        <a href="https://austinweeks.dev" target="_blank">
          austin weeks
        </a>
      </div>
    </>
  );
}
