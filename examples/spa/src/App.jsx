import React, { useState, useEffect } from "react";
import Home from "./pages/Home";
import About from "./pages/About";

export default function App() {
  const [path, setPath] = useState(window.location.pathname);

  useEffect(() => {
    const onPop = () => setPath(window.location.pathname);
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  const navigate = (to) => {
    window.history.pushState(null, "", to);
    setPath(to);
  };

  let Page;
  if (path === "/about") Page = About;
  else Page = Home;

  return (
    <div style={{ padding: 20, fontFamily: "sans-serif" }}>
      <nav style={{ marginBottom: 20 }}>
        <a
          href="/"
          onClick={(e) => {
            e.preventDefault();
            navigate("/");
          }}
          style={{ marginRight: 10 }}
        >
          Home
        </a>
        <a
          href="/about"
          onClick={(e) => {
            e.preventDefault();
            navigate("/about");
          }}
        >
          About
        </a>
      </nav>
      <Page />
    </div>
  );
}
