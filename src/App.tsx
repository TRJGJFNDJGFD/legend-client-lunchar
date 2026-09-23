import { Routes, Route } from "react-router-dom";
import { Layout } from "./components/Layout";
import { Planned } from "./components/Planned";
import { Home } from "./pages/Home";
import { Profiles } from "./pages/Profiles";
import { Logs } from "./pages/Logs";
import { Settings } from "./pages/Settings";
import { Accounts } from "./pages/Accounts";
import { Mods } from "./pages/Mods";

function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route path="/" element={<Home />} />
        <Route path="/profiles" element={<Profiles />} />
        <Route path="/logs" element={<Logs />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/mods" element={<Mods />} />
        <Route
          path="/modpacks"
          element={<Planned title="Modpacks" milestone="Milestone 3" note=".mrpack import/export." />}
        />
        <Route
          path="/resource-packs"
          element={<Planned title="Resource Packs" milestone="Milestone 3" />}
        />
        <Route path="/shaders" element={<Planned title="Shaders" milestone="Milestone 3" />} />
        <Route
          path="/cosmetics"
          element={
            <Planned
              title="Cosmetics"
              milestone="v2+"
              note="Needs a backend cosmetics service — architected via a Cosmetic API interface, not started."
            />
          }
        />
        <Route path="/servers" element={<Planned title="Servers" milestone="Milestone 2" />} />
        <Route
          path="/screenshots"
          element={<Planned title="Screenshots" milestone="Milestone 2" />}
        />
        <Route path="/accounts" element={<Accounts />} />
      </Route>
    </Routes>
  );
}

export default App;
