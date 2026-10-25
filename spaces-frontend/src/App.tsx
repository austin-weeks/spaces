import { DottedGlowBackground } from "./Background";
import Cursors from "./Cursors";

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
    </>
  );
}
